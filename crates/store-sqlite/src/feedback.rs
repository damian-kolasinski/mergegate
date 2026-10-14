//! The `events` log and each item's delivered cursor.

use mergegate_core::feedback::{Event, EventKind, FeedbackStore};
use mergegate_core::item::ItemId;
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, params};

use crate::Store;
use crate::reviews::{ReviewRow, review};

/// `?1` and `?2` name the item.
const ITEM: &str = "SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                    WHERE p.name = ?1 AND i.number = ?2";

impl FeedbackStore for Store {
    fn events(&self, id: &ItemId, after: u64) -> Result<Vec<Event>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT e.cursor, r.id, r.revision, r.verdict, r.summary, r.submitted_at
                 FROM events e JOIN reviews r ON r.id = e.review_id
                 WHERE e.item_id = ({ITEM}) AND e.cursor > ?3
                 ORDER BY e.cursor"
            ))
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(
                params![id.project().as_str(), id.number() as i64, after as i64],
                |row| {
                    let cursor: i64 = row.get(0)?;
                    let review = ReviewRow {
                        id: row.get(1)?,
                        revision: row.get(2)?,
                        verdict: row.get(3)?,
                        summary: row.get(4)?,
                        submitted_at: row.get(5)?,
                    };
                    Ok((cursor as u64, review))
                },
            )
            .map_err(StoreError::new)?;
        rows.map(|row| {
            let (cursor, row) = row.map_err(StoreError::new)?;
            Ok(Event {
                cursor,
                kind: EventKind::ReviewSubmitted(review(&self.conn, id, row)?),
            })
        })
        .collect()
    }

    fn last_cursor(&self, id: &ItemId) -> Result<u64, StoreError> {
        let cursor: i64 = self
            .conn
            .query_row(
                &format!("SELECT coalesce(max(cursor), 0) FROM events WHERE item_id = ({ITEM})"),
                params![id.project().as_str(), id.number() as i64],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        Ok(cursor as u64)
    }

    fn delivered(&self, id: &ItemId) -> Result<u64, StoreError> {
        let cursor: Option<i64> = self
            .conn
            .query_row(
                "SELECT i.delivered FROM items i JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2",
                params![id.project().as_str(), id.number() as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::new)?;
        Ok(cursor.unwrap_or(0) as u64)
    }

    fn set_delivered(&mut self, id: &ItemId, cursor: u64) -> Result<(), StoreError> {
        self.conn
            .execute(
                &format!("UPDATE items SET delivered = ?3 WHERE id = ({ITEM})"),
                params![id.project().as_str(), id.number() as i64, cursor as i64],
            )
            .map_err(StoreError::new)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
