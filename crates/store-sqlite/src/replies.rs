//! The `replies` table: each sent comment's thread. `ReviewStore`'s impl in
//! `reviews` calls these, as a review sends the operator's drafts.

use mergegate_core::item::ItemId;
use mergegate_core::project::ProjectName;
use mergegate_core::review::{Author, CommentId, NewReply, Reply, ReplyStatus};
use mergegate_core::store::StoreError;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::items::NOW;

const COLUMNS: &str = "id, comment_id, author, status, review_id, revision, body, created_at";

pub(crate) fn insert(conn: &Connection, reply: NewReply) -> Result<Reply, StoreError> {
    let row = conn
        .query_row(
            &format!(
                "INSERT INTO replies (comment_id, author, status, revision, body, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, {NOW})
                 RETURNING {COLUMNS}"
            ),
            params![
                reply.comment.0 as i64,
                reply.author.as_str(),
                reply.status.map(ReplyStatus::as_str),
                reply.revision,
                reply.body,
            ],
            ReplyRow::read,
        )
        .map_err(StoreError::new)?;
    row.into_reply(&reply.item)
}

/// Every reply on `id`'s comments, oldest first; only those `review` sent
/// when given.
pub(crate) fn of_item(
    conn: &Connection,
    id: &ItemId,
    review: Option<u64>,
) -> Result<Vec<Reply>, StoreError> {
    let mut stmt = conn
        .prepare_cached(&format!(
            "SELECT {COLUMNS} FROM replies
             WHERE comment_id IN (
                 SELECT c.id FROM comments c JOIN items i ON i.id = c.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2
             ) AND (?3 IS NULL OR review_id = ?3)
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
            ReplyRow::read,
        )
        .map_err(StoreError::new)?;
    rows.map(|row| row.map_err(StoreError::new)?.into_reply(id))
        .collect()
}

/// The reply, on whichever item.
pub(crate) fn one(conn: &Connection, id: u64) -> Result<Option<Reply>, StoreError> {
    let item: Option<(String, i64)> = conn
        .query_row(
            "SELECT p.name, i.number FROM replies r JOIN comments c ON c.id = r.comment_id
             JOIN items i ON i.id = c.item_id JOIN projects p ON p.id = i.project_id
             WHERE r.id = ?1",
            params![id as i64],
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
            &format!("SELECT {COLUMNS} FROM replies WHERE id = ?1"),
            params![id as i64],
            ReplyRow::read,
        )
        .map_err(StoreError::new)?;
    row.into_reply(&item).map(Some)
}

/// Sends the operator's draft replies on `item_id`'s comments with the review.
pub(crate) fn attach(conn: &Connection, item_id: i64, review: i64) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE replies SET review_id = ?2
         WHERE author = 'operator' AND review_id IS NULL
           AND comment_id IN (SELECT id FROM comments WHERE item_id = ?1)",
        params![item_id, review],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

pub(crate) fn set_body(conn: &Connection, id: u64, body: &str) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE replies SET body = ?2
         WHERE id = ?1 AND author = 'operator' AND review_id IS NULL",
        params![id as i64, body],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: u64) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM replies WHERE id = ?1 AND author = 'operator' AND review_id IS NULL",
        params![id as i64],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

/// A `replies` row as read, before its author and status are checked.
struct ReplyRow {
    id: i64,
    comment: i64,
    author: String,
    status: Option<String>,
    review: Option<i64>,
    revision: u32,
    body: String,
    created_at: String,
}

impl ReplyRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            comment: row.get(1)?,
            author: row.get(2)?,
            status: row.get(3)?,
            review: row.get(4)?,
            revision: row.get(5)?,
            body: row.get(6)?,
            created_at: row.get(7)?,
        })
    }

    fn into_reply(self, item: &ItemId) -> Result<Reply, StoreError> {
        let author = Author::ALL
            .into_iter()
            .find(|a| a.as_str() == self.author)
            .ok_or_else(|| StoreError::new(format!("unknown author {:?}", self.author)))?;
        let status = match self.status {
            None => None,
            Some(name) => Some(
                ReplyStatus::ALL
                    .into_iter()
                    .find(|s| s.as_str() == name)
                    .ok_or_else(|| StoreError::new(format!("unknown reply status {name:?}")))?,
            ),
        };
        Ok(Reply {
            id: self.id as u64,
            comment: CommentId(self.comment as u64),
            item: item.clone(),
            author,
            status,
            body: self.body,
            revision: self.revision,
            review: self.review.map(|r| r as u64),
            created_at: self.created_at,
        })
    }
}

#[cfg(test)]
#[path = "replies_tests.rs"]
mod tests;
