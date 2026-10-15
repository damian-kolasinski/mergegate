//! The `reviews` table, and the events that carry reviews to agents.

use mergegate_core::item::{ItemId, ItemState};
use mergegate_core::review::{
    Comment, CommentId, NewComment, NewReply, NewReview, Reply, Review, ReviewStore, Verdict,
};
use mergegate_core::store::StoreError;
use rusqlite::{Connection, params};

use crate::items::NOW;
use crate::{Store, comments, replies};

impl ReviewStore for Store {
    fn insert_review(&mut self, review: NewReview, state: ItemState) -> Result<Review, StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let item_id: i64 = tx
            .query_row(
                "SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2",
                params![review.item.project().as_str(), review.item.number() as i64],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        let (id, submitted_at): (i64, String) = tx
            .query_row(
                &format!(
                    "INSERT INTO reviews (item_id, revision, verdict, summary, submitted_at)
                     VALUES (?1, ?2, ?3, ?4, {NOW}) RETURNING id, submitted_at"
                ),
                params![
                    item_id,
                    review.revision,
                    review.verdict.as_str(),
                    review.summary
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(StoreError::new)?;
        tx.execute(
            "UPDATE items SET state = ?2 WHERE id = ?1",
            params![item_id, state.as_str()],
        )
        .map_err(StoreError::new)?;
        tx.execute(
            "INSERT INTO events (item_id, type, revision, source, review_id)
             VALUES (?1, 'review.submitted', ?2, 'operator', ?3)",
            params![item_id, review.revision, id],
        )
        .map_err(StoreError::new)?;
        comments::attach(&tx, item_id, id)?;
        replies::attach(&tx, item_id, id)?;
        let sent = comments::of_item(&tx, &review.item, Some(id as u64))?;
        let answers = replies::of_item(&tx, &review.item, Some(id as u64))?;
        tx.commit().map_err(StoreError::new)?;
        Ok(Review {
            id: id as u64,
            item: review.item,
            revision: review.revision,
            verdict: review.verdict,
            summary: review.summary,
            comments: sent,
            replies: answers,
            submitted_at,
        })
    }

    fn last_reviewed(&self, id: &ItemId) -> Result<Option<u32>, StoreError> {
        self.conn
            .prepare_cached(
                "SELECT max(r.revision) FROM reviews r
                 JOIN items i ON i.id = r.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2",
            )
            .map_err(StoreError::new)?
            .query_row(params![id.project().as_str(), id.number() as i64], |row| {
                row.get(0)
            })
            .map_err(StoreError::new)
    }

    fn insert_draft(&mut self, comment: NewComment) -> Result<Comment, StoreError> {
        comments::insert(&self.conn, comment)
    }

    fn comments(&self, id: &ItemId) -> Result<Vec<Comment>, StoreError> {
        comments::of_item(&self.conn, id, None)
    }

    fn set_draft_body(&mut self, id: CommentId, body: &str) -> Result<(), StoreError> {
        comments::set_body(&self.conn, id, body)
    }

    fn delete_draft(&mut self, id: CommentId) -> Result<(), StoreError> {
        comments::delete(&self.conn, id)
    }

    fn comment(&self, id: CommentId) -> Result<Option<Comment>, StoreError> {
        comments::one(&self.conn, id)
    }

    fn insert_reply(&mut self, reply: NewReply) -> Result<Reply, StoreError> {
        replies::insert(&self.conn, reply)
    }

    fn reply(&self, id: u64) -> Result<Option<Reply>, StoreError> {
        replies::one(&self.conn, id)
    }

    fn set_draft_reply_body(&mut self, id: u64, body: &str) -> Result<(), StoreError> {
        replies::set_body(&self.conn, id, body)
    }

    fn delete_draft_reply(&mut self, id: u64) -> Result<(), StoreError> {
        replies::delete(&self.conn, id)
    }

    fn replies(&self, id: &ItemId) -> Result<Vec<Reply>, StoreError> {
        replies::of_item(&self.conn, id, None)
    }
}

pub(crate) fn verdict_named(name: &str) -> Result<Verdict, StoreError> {
    Verdict::ALL
        .into_iter()
        .find(|v| v.as_str() == name)
        .ok_or_else(|| StoreError::new(format!("unknown verdict {name:?}")))
}

/// For reads that already know the item.
pub(crate) fn review(
    conn: &Connection,
    item: &ItemId,
    row: ReviewRow,
) -> Result<Review, StoreError> {
    Ok(Review {
        id: row.id as u64,
        item: item.clone(),
        revision: row.revision,
        verdict: verdict_named(&row.verdict)?,
        summary: row.summary,
        comments: comments::of_item(conn, item, Some(row.id as u64))?,
        replies: replies::of_item(conn, item, Some(row.id as u64))?,
        submitted_at: row.submitted_at,
    })
}

/// A `reviews` row as read, before its verdict is checked.
pub(crate) struct ReviewRow {
    pub id: i64,
    pub revision: u32,
    pub verdict: String,
    pub summary: String,
    pub submitted_at: String,
}

#[cfg(test)]
#[path = "reviews_tests.rs"]
mod tests;
