//! The `interdiffs` and `interdiff_files` tables.

use mergegate_core::interdiff::{Change, InterdiffFile, InterdiffStore, NewInterdiff};
use mergegate_core::item::{ChangedFile, FileStat, ItemId};
use mergegate_core::store::StoreError;
use mergegate_core::triage::Triage;
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{Store, modes};

impl InterdiffStore for Store {
    fn interdiff(
        &self,
        id: &ItemId,
        from: u32,
        to: u32,
    ) -> Result<Option<Vec<InterdiffFile>>, StoreError> {
        let item_id: Option<i64> = self
            .conn
            .prepare_cached(
                "SELECT d.item_id FROM interdiffs d
                 JOIN items i ON i.id = d.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2
                     AND d.from_revision = ?3 AND d.to_revision = ?4",
            )
            .map_err(StoreError::new)?
            .query_row(
                params![id.project().as_str(), id.number() as i64, from, to],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::new)?;
        let Some(item_id) = item_id else {
            return Ok(None);
        };
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT path, old_path, old_blob, new_blob, additions, deletions, change,
                        old_mode, new_mode
                 FROM interdiff_files
                 WHERE item_id = ?1 AND from_revision = ?2 AND to_revision = ?3
                 ORDER BY position",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(params![item_id, from, to], |row| {
                let [old_mode, new_mode] = match modes::read(row, 7)? {
                    Ok(modes) => modes,
                    Err(e) => return Ok(Err(e)),
                };
                let stat = FileStat {
                    file: ChangedFile {
                        path: row.get(0)?,
                        old_path: row.get(1)?,
                        old_blob: row.get(2)?,
                        new_blob: row.get(3)?,
                        old_mode,
                        new_mode,
                    },
                    additions: row.get::<_, Option<i64>>(4)?.map(|n| n as u64),
                    deletions: row.get::<_, Option<i64>>(5)?.map(|n| n as u64),
                    // A file's triage is its revision's, which rules change.
                    generated: false,
                    triage: Triage::default(),
                };
                Ok(Ok((stat, row.get::<_, String>(6)?)))
            })
            .map_err(StoreError::new)?;
        rows.map(|row| {
            let (stat, change) = row.map_err(StoreError::new)??;
            let change = Change::ALL
                .into_iter()
                .find(|c| c.as_str() == change)
                .ok_or_else(|| StoreError::new(format!("unknown interdiff change {change:?}")))?;
            Ok(InterdiffFile { stat, change })
        })
        .collect::<Result<_, _>>()
        .map(Some)
    }

    fn interdiffs_to(&self, id: &ItemId, to: u32) -> Result<Vec<u32>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT d.from_revision FROM interdiffs d
                 JOIN items i ON i.id = d.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2 AND d.to_revision = ?3
                 ORDER BY d.from_revision",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(
                params![id.project().as_str(), id.number() as i64, to],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        rows.collect::<Result<_, _>>().map_err(StoreError::new)
    }
}

/// Records the interdiffs to revision `to`, in the transaction that writes it.
pub(crate) fn insert(
    tx: &Transaction,
    item_id: i64,
    to: i64,
    interdiffs: &[NewInterdiff],
) -> Result<(), StoreError> {
    let mut interdiff = tx
        .prepare_cached(
            "INSERT INTO interdiffs (item_id, from_revision, to_revision) VALUES (?1, ?2, ?3)",
        )
        .map_err(StoreError::new)?;
    let mut file = tx
        .prepare_cached(
            "INSERT INTO interdiff_files
                 (item_id, from_revision, to_revision, position, path, old_path, old_blob,
                  new_blob, additions, deletions, change, old_mode, new_mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )
        .map_err(StoreError::new)?;
    for new in interdiffs {
        interdiff
            .execute(params![item_id, new.from, to])
            .map_err(StoreError::new)?;
        for (position, f) in new.files.iter().enumerate() {
            let [old_mode, new_mode] = modes::columns(&f.stat.file);
            file.execute(params![
                item_id,
                new.from,
                to,
                position as i64,
                f.stat.file.path,
                f.stat.file.old_path,
                f.stat.file.old_blob,
                f.stat.file.new_blob,
                f.stat.additions.map(|n| n as i64),
                f.stat.deletions.map(|n| n as i64),
                f.change.as_str(),
                old_mode,
                new_mode,
            ])
            .map_err(StoreError::new)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "interdiffs_tests.rs"]
mod tests;
