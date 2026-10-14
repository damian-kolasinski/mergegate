//! The `comments` table. `ReviewStore`'s impl in `reviews` calls these, as a
//! review sends the drafts, and its tests cover them.

use mergegate_core::item::ItemId;
use mergegate_core::project::ProjectName;
use mergegate_core::review::{Comment, CommentId, NewComment, Side};
use mergegate_core::store::StoreError;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::items::NOW;

/// `?1` and `?2` name the item.
const ITEM: &str = "SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                    WHERE p.name = ?1 AND i.number = ?2";

const COLUMNS: &str =
    "id, revision, review_id, path, side, start_line, line, body, quote, created_at";

pub(crate) fn insert(conn: &Connection, comment: NewComment) -> Result<Comment, StoreError> {
    let row = conn
        .query_row(
            &format!(
                "INSERT INTO comments
                     (item_id, revision, path, side, start_line, line, body, quote, created_at)
                 VALUES (({ITEM}), ?3, ?4, ?5, ?6, ?7, ?8, ?9, {NOW})
                 RETURNING {COLUMNS}"
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
            CommentRow::read,
        )
        .map_err(StoreError::new)?;
    row.into_comment(&comment.item)
}

/// `id`'s comments, oldest first; only those `review` sent when given.
pub(crate) fn of_item(
    conn: &Connection,
    id: &ItemId,
    review: Option<u64>,
) -> Result<Vec<Comment>, StoreError> {
    let mut stmt = conn
        .prepare_cached(&format!(
            "SELECT {COLUMNS} FROM comments
             WHERE item_id = ({ITEM}) AND (?3 IS NULL OR review_id = ?3)
             ORDER BY id"
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
    let row = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM comments WHERE id = ?1"),
            params![id.0 as i64],
            CommentRow::read,
        )
        .map_err(StoreError::new)?;
    row.into_comment(&item).map(Some)
}

/// Sends `item_id`'s drafts with the review.
pub(crate) fn attach(conn: &Connection, item_id: i64, review: i64) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE comments SET review_id = ?2 WHERE item_id = ?1 AND review_id IS NULL",
        params![item_id, review],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

pub(crate) fn set_body(conn: &Connection, id: CommentId, body: &str) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE comments SET body = ?2 WHERE id = ?1 AND review_id IS NULL",
        params![id.0 as i64, body],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: CommentId) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM comments WHERE id = ?1 AND review_id IS NULL",
        params![id.0 as i64],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

/// A `comments` row as read, before its side is checked.
struct CommentRow {
    id: i64,
    revision: u32,
    review: Option<i64>,
    path: String,
    side: String,
    start_line: u32,
    line: u32,
    body: String,
    quote: String,
    created_at: String,
}

impl CommentRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
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
        })
    }

    fn into_comment(self, item: &ItemId) -> Result<Comment, StoreError> {
        let side = Side::ALL
            .into_iter()
            .find(|s| s.as_str() == self.side)
            .ok_or_else(|| StoreError::new(format!("unknown side {:?}", self.side)))?;
        Ok(Comment {
            id: CommentId(self.id as u64),
            item: item.clone(),
            revision: self.revision,
            path: self.path,
            side,
            start_line: self.start_line,
            line: self.line,
            body: self.body,
            quote: self.quote,
            review: self.review.map(|r| r as u64),
            created_at: self.created_at,
        })
    }
}
