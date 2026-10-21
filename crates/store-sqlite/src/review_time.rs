//! The `review_time` table: the operator's active time per turn and
//! revision, and the reviews that end the turns.

use mergegate_core::item::ItemId;
use mergegate_core::review_time::{Logged, ReviewTimeStore, SentReview};
use mergegate_core::store::StoreError;
use rusqlite::params;

use crate::Store;
use crate::reviews::verdict_named;

impl ReviewTimeStore for Store {
    fn review_time(&self, id: &ItemId) -> Result<Vec<Logged>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT t.turn, t.revision, t.ms FROM review_time t
                 JOIN items i ON i.id = t.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2
                 ORDER BY t.turn, t.revision",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(params![id.project().as_str(), id.number() as i64], |row| {
                Ok(Logged {
                    turn: row.get(0)?,
                    revision: row.get(1)?,
                    ms: row.get::<_, i64>(2)? as u64,
                })
            })
            .map_err(StoreError::new)?;
        rows.collect::<Result<_, _>>().map_err(StoreError::new)
    }

    fn add_review_time(&mut self, id: &ItemId, logged: Logged) -> Result<(), StoreError> {
        let ms = i64::try_from(logged.ms).map_err(StoreError::new)?;
        self.conn
            .prepare_cached(
                "INSERT INTO review_time (item_id, turn, revision, ms)
                 SELECT i.id, ?3, ?4, ?5 FROM items i JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2
                 ON CONFLICT (item_id, turn, revision) DO UPDATE SET ms = ms + excluded.ms",
            )
            .map_err(StoreError::new)?
            .execute(params![
                id.project().as_str(),
                id.number() as i64,
                logged.turn,
                logged.revision,
                ms
            ])
            .map_err(StoreError::new)?;
        Ok(())
    }

    fn sent_reviews(&self, id: &ItemId) -> Result<Vec<SentReview>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT r.revision, r.verdict, r.submitted_at FROM reviews r
                 JOIN items i ON i.id = r.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2 AND r.carried_from IS NULL
                 ORDER BY r.id",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(params![id.project().as_str(), id.number() as i64], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(StoreError::new)?;
        rows.map(|row| {
            let (revision, verdict, submitted_at) = row.map_err(StoreError::new)?;
            Ok(SentReview {
                revision,
                verdict: verdict_named(&verdict)?,
                submitted_at,
            })
        })
        .collect()
    }
}

#[cfg(test)]
#[path = "review_time_tests.rs"]
mod tests;
