//! The `conflicts` table: open items test-merged into each other.

use mergegate_core::conflict::{ConflictStore, Pair};
use mergegate_core::item::ItemId;
use mergegate_core::project::ProjectName;
use mergegate_core::store::StoreError;
use rusqlite::params;

use crate::Store;

impl ConflictStore for Store {
    fn pairs(&self, project: Option<&ProjectName>) -> Result<Vec<Pair>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT p.name, i.number, c.item_head, o.number, c.other_head, c.conflicts
                 FROM conflicts c
                 JOIN items i ON i.id = c.item_id
                 JOIN items o ON o.id = c.other_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE ?1 IS NULL OR p.name = ?1
                 ORDER BY p.name, i.number, o.number",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(params![project.map(ProjectName::as_str)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, bool>(5)?,
                ))
            })
            .map_err(StoreError::new)?;
        let mut pairs = Vec::new();
        for row in rows {
            let (project, item, item_head, other, other_head, conflicts) =
                row.map_err(StoreError::new)?;
            let project = ProjectName::parse(&project).map_err(StoreError::new)?;
            pairs.push(Pair {
                item: ItemId::new(project.clone(), item as u64),
                item_head,
                other: ItemId::new(project, other as u64),
                other_head,
                conflicts,
            });
        }
        Ok(pairs)
    }

    fn set_pairs(&mut self, project: &ProjectName, pairs: &[Pair]) -> Result<(), StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        tx.execute(
            "DELETE FROM conflicts WHERE item_id IN (
                 SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1)",
            params![project.as_str()],
        )
        .map_err(StoreError::new)?;
        {
            let mut insert = tx
                .prepare_cached(
                    "INSERT INTO conflicts (item_id, other_id, item_head, other_head, conflicts)
                     SELECT i.id, o.id, ?4, ?5, ?6
                     FROM projects p
                     JOIN items i ON i.project_id = p.id AND i.number = ?2
                     JOIN items o ON o.project_id = p.id AND o.number = ?3
                     WHERE p.name = ?1",
                )
                .map_err(StoreError::new)?;
            for pair in pairs {
                insert
                    .execute(params![
                        project.as_str(),
                        pair.item.number() as i64,
                        pair.other.number() as i64,
                        pair.item_head,
                        pair.other_head,
                        pair.conflicts,
                    ])
                    .map_err(StoreError::new)?;
            }
        }
        tx.commit().map_err(StoreError::new)
    }
}

#[cfg(test)]
#[path = "conflicts_tests.rs"]
mod tests;
