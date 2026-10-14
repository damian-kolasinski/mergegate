//! The `replies` table: each sent comment's thread. `ReviewStore`'s impl in
//! `reviews` calls these.

use mergegate_core::item::ItemId;
use mergegate_core::review::{Author, CommentId, NewReply, Reply, ReplyStatus};
use mergegate_core::store::StoreError;
use rusqlite::{Connection, Row, params};

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

/// Every reply on `id`'s comments, oldest first.
pub(crate) fn of_item(conn: &Connection, id: &ItemId) -> Result<Vec<Reply>, StoreError> {
    let mut stmt = conn
        .prepare_cached(&format!(
            "SELECT {COLUMNS} FROM replies
             WHERE comment_id IN (
                 SELECT c.id FROM comments c JOIN items i ON i.id = c.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2
             )
             ORDER BY id"
        ))
        .map_err(StoreError::new)?;
    let rows = stmt
        .query_map(
            params![id.project().as_str(), id.number() as i64],
            ReplyRow::read,
        )
        .map_err(StoreError::new)?;
    rows.map(|row| row.map_err(StoreError::new)?.into_reply(id))
        .collect()
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
