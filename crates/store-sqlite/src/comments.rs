//! The `comments` table, with what `external_comments` adds to an external
//! one. `ReviewStore`'s impl in `reviews` calls these, as a review sends the
//! drafts, and its tests cover them.

use mergegate_core::external::{External, Vote};
use mergegate_core::github::PullCommentKind;
use mergegate_core::item::ItemId;
use mergegate_core::project::ProjectName;
use mergegate_core::review::{Anchor, Comment, CommentId, MovedDraft, NewComment, Side};
use mergegate_core::store::StoreError;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::items::NOW;

/// `?1` and `?2` name the item.
const ITEM: &str = "SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                    WHERE p.name = ?1 AND i.number = ?2";

/// A comment with what GitHub says about it, as `c` and `x`.
pub(crate) const FROM: &str = "comments c LEFT JOIN external_comments x ON x.comment_id = c.id";

/// The columns [`CommentRow::read`] reads, `body` spelling the body's.
pub(crate) fn columns(body: &str) -> String {
    format!(
        "c.id, c.revision, c.review_id, c.path, c.side, c.start_line, c.line, {body}, c.quote,
         c.created_at, x.kind, x.github_id, x.login, x.bot, x.url, x.path,
         x.pending_at IS NOT NULL, x.agreed_reply, x.vote, x.summary"
    )
}

/// How many [`columns`] there are, the index of one a query adds after them.
pub(crate) const COLUMNS: usize = 20;

pub(crate) fn insert(conn: &Connection, comment: NewComment) -> Result<Comment, StoreError> {
    let id: i64 = conn
        .query_row(
            &format!(
                "INSERT INTO comments (item_id, revision, source, path, side, start_line, line,
                                       body, quote, created_at)
                 VALUES (({ITEM}), ?3, 'operator', ?4, ?5, ?6, ?7, ?8, ?9, {NOW})
                 RETURNING id"
            ),
            params![
                comment.item.project().as_str(),
                comment.item.number() as i64,
                comment.revision,
                comment.path,
                comment.side.as_str(),
                comment.start_line,
                comment.line,
                comment.body,
                comment.quote,
            ],
            |row| row.get(0),
        )
        .map_err(StoreError::new)?;
    on_item(conn, &comment.item, id)
}

/// `id`'s comments, oldest first; only those `review` sent when given.
pub(crate) fn of_item(
    conn: &Connection,
    id: &ItemId,
    review: Option<u64>,
) -> Result<Vec<Comment>, StoreError> {
    let mut stmt = conn
        .prepare_cached(&format!(
            "SELECT {} FROM {FROM}
             WHERE c.item_id = ({ITEM}) AND (?3 IS NULL OR c.review_id = ?3)
             ORDER BY c.id",
            columns("c.body")
        ))
        .map_err(StoreError::new)?;
    let rows = stmt
        .query_map(
            params![
                id.project().as_str(),
                id.number() as i64,
                review.map(|r| r as i64)
            ],
            CommentRow::read,
        )
        .map_err(StoreError::new)?;
    rows.map(|row| row.map_err(StoreError::new)?.into_comment(id))
        .collect()
}

/// The comment, on whichever item.
pub(crate) fn one(conn: &Connection, id: CommentId) -> Result<Option<Comment>, StoreError> {
    let item: Option<(String, i64)> = conn
        .query_row(
            "SELECT p.name, i.number FROM comments c
             JOIN items i ON i.id = c.item_id JOIN projects p ON p.id = i.project_id
             WHERE c.id = ?1",
            params![id.0 as i64],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(StoreError::new)?;
    let Some((project, number)) = item else {
        return Ok(None);
    };
    let item = ItemId::new(
        ProjectName::parse(&project).map_err(StoreError::new)?,
        number as u64,
    );
    on_item(conn, &item, id.0 as i64).map(Some)
}

/// The comment, which is on `item`.
pub(crate) fn on_item(conn: &Connection, item: &ItemId, id: i64) -> Result<Comment, StoreError> {
    let row = conn
        .query_row(
            &format!("SELECT {} FROM {FROM} WHERE c.id = ?1", columns("c.body")),
            params![id],
            CommentRow::read,
        )
        .map_err(StoreError::new)?;
    row.into_comment(item)
}

/// Sends `item_id`'s drafts with the review, the `moved` ones on their new
/// lines.
pub(crate) fn attach(
    conn: &Connection,
    item_id: i64,
    review: i64,
    moved: &[MovedDraft],
) -> Result<(), StoreError> {
    let mut place = conn
        .prepare_cached(
            "UPDATE comments SET revision = ?3, start_line = ?4, line = ?5
             WHERE id = ?1 AND item_id = ?2 AND source = 'operator' AND review_id IS NULL",
        )
        .map_err(StoreError::new)?;
    for draft in moved {
        place
            .execute(params![
                draft.comment.0 as i64,
                item_id,
                draft.revision,
                draft.start_line,
                draft.line
            ])
            .map_err(StoreError::new)?;
    }
    conn.execute(
        "UPDATE comments SET review_id = ?2
         WHERE item_id = ?1 AND source = 'operator' AND review_id IS NULL",
        params![item_id, review],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

pub(crate) fn set_body(conn: &Connection, id: CommentId, body: &str) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE comments SET body = ?2
         WHERE id = ?1 AND source = 'operator' AND review_id IS NULL",
        params![id.0 as i64, body],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

pub(crate) fn set_anchor(
    conn: &Connection,
    id: CommentId,
    revision: u32,
    anchor: &Anchor,
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE comments SET revision = ?2, path = ?3, side = ?4, start_line = ?5, line = ?6,
                             quote = ?7
         WHERE id = ?1 AND source = 'operator' AND review_id IS NULL",
        params![
            id.0 as i64,
            revision,
            anchor.path,
            anchor.side.as_str(),
            anchor.start_line,
            anchor.line,
            anchor.quote
        ],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: CommentId) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM comments WHERE id = ?1 AND source = 'operator' AND review_id IS NULL",
        params![id.0 as i64],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

/// A comment's row as read, before its side and kind are checked.
pub(crate) struct CommentRow {
    id: i64,
    revision: u32,
    review: Option<i64>,
    path: Option<String>,
    side: Option<String>,
    start_line: Option<u32>,
    line: Option<u32>,
    body: String,
    quote: Option<String>,
    created_at: String,
    external: Option<ExternalRow>,
}

struct ExternalRow {
    kind: String,
    github_id: i64,
    login: String,
    bot: bool,
    url: String,
    path: Option<String>,
    pending: bool,
    agreed_reply: Option<i64>,
    vote: Option<String>,
    summary: bool,
}

impl CommentRow {
    pub(crate) fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        let external = match row.get::<_, Option<String>>(10)? {
            Some(kind) => Some(ExternalRow {
                kind,
                github_id: row.get(11)?,
                login: row.get(12)?,
                bot: row.get(13)?,
                url: row.get(14)?,
                path: row.get(15)?,
                pending: row.get(16)?,
                agreed_reply: row.get(17)?,
                vote: row.get(18)?,
                summary: row.get(19)?,
            }),
            None => None,
        };
        Ok(Self {
            id: row.get(0)?,
            revision: row.get(1)?,
            review: row.get(2)?,
            path: row.get(3)?,
            side: row.get(4)?,
            start_line: row.get(5)?,
            line: row.get(6)?,
            body: row.get(7)?,
            quote: row.get(8)?,
            created_at: row.get(9)?,
            external,
        })
    }

    pub(crate) fn into_comment(self, item: &ItemId) -> Result<Comment, StoreError> {
        let anchor = match (self.path, self.side, self.start_line, self.line, self.quote) {
            (Some(path), Some(side), Some(start_line), Some(line), Some(quote)) => Some(Anchor {
                path,
                side: Side::ALL
                    .into_iter()
                    .find(|s| s.as_str() == side)
                    .ok_or_else(|| StoreError::new(format!("unknown side {side:?}")))?,
                start_line,
                line,
                quote,
            }),
            (None, None, None, None, None) => None,
            _ => {
                return Err(StoreError::new(format!(
                    "comment {} half anchored",
                    self.id
                )));
            }
        };
        let external = match self.external {
            Some(row) => Some(External {
                vote: row
                    .vote
                    .map(|vote| {
                        Vote::parse(&vote)
                            .ok_or_else(|| StoreError::new(format!("unknown vote {vote:?}")))
                    })
                    .transpose()?,
                kind: PullCommentKind::ALL
                    .into_iter()
                    .find(|k| k.as_str() == row.kind)
                    .ok_or_else(|| StoreError::new(format!("unknown kind {:?}", row.kind)))?,
                github_id: row.github_id as u64,
                login: row.login,
                bot: row.bot,
                url: row.url,
                path: row.path,
                pending: row.pending,
                summary: row.summary,
                agreed_reply: row.agreed_reply.map(|r| r as u64),
            }),
            None => None,
        };
        Ok(Comment {
            id: CommentId(self.id as u64),
            item: item.clone(),
            revision: self.revision,
            anchor,
            body: self.body,
            review: self.review.map(|r| r as u64),
            external,
            created_at: self.created_at,
        })
    }
}
