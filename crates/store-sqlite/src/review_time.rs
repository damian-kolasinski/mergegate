//! The `review_time` table: the operator's active time per revision.

use std::collections::BTreeMap;

use mergegate_core::item::ItemId;
use mergegate_core::review_time::ReviewTimeStore;
use mergegate_core::store::StoreError;
use rusqlite::params;

use crate::Store;

impl ReviewTimeStore for Store {
    fn review_time(&self, id: &ItemId) -> Result<BTreeMap<u32, u64>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT t.revision, t.ms FROM review_time t
                 JOIN items i ON i.id = t.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(params![id.project().as_str(), id.number() as i64], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, i64>(1)? as u64))
            })
            .map_err(StoreError::new)?;
        rows.collect::<Result<_, _>>().map_err(StoreError::new)
    }

    fn add_review_time(&mut self, id: &ItemId, revision: u32, ms: u64) -> Result<(), StoreError> {
        let ms = i64::try_from(ms).map_err(StoreError::new)?;
        self.conn
            .prepare_cached(
                "INSERT INTO review_time (item_id, revision, ms)
                 SELECT i.id, ?3, ?4 FROM items i JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2
                 ON CONFLICT (item_id, revision) DO UPDATE SET ms = ms + excluded.ms",
            )
            .map_err(StoreError::new)?
            .execute(params![
                id.project().as_str(),
                id.number() as i64,
                revision,
                ms
            ])
            .map_err(StoreError::new)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "review_time_tests.rs"]
mod tests;
