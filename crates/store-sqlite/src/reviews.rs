//! The `reviews` table, and the events that carry reviews to agents.

use mergegate_core::item::{ItemId, ItemState};
use mergegate_core::review::{NewReview, Review, ReviewStore, Verdict};
use mergegate_core::store::StoreError;
use rusqlite::params;

use crate::Store;
use crate::items::NOW;

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
            "INSERT INTO events (item_id, type, review_id) VALUES (?1, 'review.submitted', ?2)",
            params![item_id, id],
        )
        .map_err(StoreError::new)?;
        tx.commit().map_err(StoreError::new)?;
        Ok(Review {
            id: id as u64,
            item: review.item,
            revision: review.revision,
            verdict: review.verdict,
            summary: review.summary,
            submitted_at,
        })
    }
}

pub(crate) fn verdict_named(name: &str) -> Result<Verdict, StoreError> {
    Verdict::ALL
        .into_iter()
        .find(|v| v.as_str() == name)
        .ok_or_else(|| StoreError::new(format!("unknown verdict {name:?}")))
}

/// For reads that already know the item.
pub(crate) fn review(item: &ItemId, row: ReviewRow) -> Result<Review, StoreError> {
    Ok(Review {
        id: row.id as u64,
        item: item.clone(),
        revision: row.revision,
        verdict: verdict_named(&row.verdict)?,
        summary: row.summary,
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
