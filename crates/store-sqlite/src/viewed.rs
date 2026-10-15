//! The `viewed` table: the files marked viewed, per revision.

use std::collections::BTreeSet;

use mergegate_core::item::ItemId;
use mergegate_core::store::StoreError;
use mergegate_core::viewed::ViewedStore;
use rusqlite::{Transaction, params};

use crate::Store;

impl ViewedStore for Store {
    fn viewed(&self, id: &ItemId, revision: u32) -> Result<BTreeSet<String>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT v.path FROM viewed v
                 JOIN items i ON i.id = v.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2 AND v.revision = ?3",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(
                params![id.project().as_str(), id.number() as i64, revision],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        rows.collect::<Result<_, _>>().map_err(StoreError::new)
    }

    fn set_viewed(
        &mut self,
        id: &ItemId,
        revision: u32,
        path: &str,
        viewed: bool,
    ) -> Result<(), StoreError> {
        let sql = if viewed {
            "INSERT OR IGNORE INTO viewed (item_id, revision, path)
             SELECT i.id, ?3, ?4 FROM items i JOIN projects p ON p.id = i.project_id
             WHERE p.name = ?1 AND i.number = ?2"
        } else {
            "DELETE FROM viewed WHERE revision = ?3 AND path = ?4 AND item_id = (
                 SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2)"
        };
        self.conn
            .prepare_cached(sql)
            .map_err(StoreError::new)?
            .execute(params![
                id.project().as_str(),
                id.number() as i64,
                revision,
                path
            ])
            .map_err(StoreError::new)?;
        Ok(())
    }
}

/// Marks `paths` viewed on a revision as it's written: the marks carried over
/// from an earlier revision.
pub(crate) fn insert(
    tx: &Transaction,
    item_id: i64,
    revision: i64,
    paths: &[String],
) -> Result<(), StoreError> {
    let mut stmt = tx
        .prepare_cached(
            "INSERT OR IGNORE INTO viewed (item_id, revision, path) VALUES (?1, ?2, ?3)",
        )
        .map_err(StoreError::new)?;
    for path in paths {
        stmt.execute(params![item_id, revision, path])
            .map_err(StoreError::new)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "viewed_tests.rs"]
mod tests;
