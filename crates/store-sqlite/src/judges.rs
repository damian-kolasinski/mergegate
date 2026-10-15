//! The `judges`, `judged` and `judged_files` tables.

use mergegate_core::item::ItemId;
use mergegate_core::judge::{JudgeSetting, JudgeStore, Provider, Verdict};
use mergegate_core::project::ProjectName;
use mergegate_core::store::StoreError;
use mergegate_core::triage::OrderBy;
use rusqlite::{OptionalExtension, params};

use crate::Store;

/// The item's row ID from `?1` (project) and `?2` (number).
const ITEM: &str = "SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                    WHERE p.name = ?1 AND i.number = ?2";

impl JudgeStore for Store {
    fn judge_setting(&self, project: &ProjectName) -> Result<Option<JudgeSetting>, StoreError> {
        let row = self
            .conn
            .prepare_cached(
                "SELECT j.provider, j.model, j.command, j.threshold, j.ordering FROM judges j
                 JOIN projects p ON p.id = j.project_id WHERE p.name = ?1",
            )
            .map_err(StoreError::new)?
            .query_row([project.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .optional()
            .map_err(StoreError::new)?;
        let Some((provider, model, command, threshold, order)) = row else {
            return Ok(None);
        };
        let provider = match (provider.as_str(), model, command) {
            ("jev", Some(model), _) => Provider::Jev { model },
            ("command", _, Some(command)) => Provider::Command { command },
            _ => return Err(StoreError::new(format!("{project} has judge {provider:?}"))),
        };
        let order = OrderBy::parse(&order)
            .ok_or_else(|| StoreError::new(format!("{project} has order {order:?}")))?;
        Ok(Some(JudgeSetting {
            provider,
            threshold,
            order,
        }))
    }

    fn set_judge_setting(
        &mut self,
        project: &ProjectName,
        setting: Option<&JudgeSetting>,
    ) -> Result<(), StoreError> {
        let Some(setting) = setting else {
            self.conn
                .prepare_cached(
                    "DELETE FROM judges
                     WHERE project_id = (SELECT id FROM projects WHERE name = ?1)",
                )
                .map_err(StoreError::new)?
                .execute([project.as_str()])
                .map_err(StoreError::new)?;
            return Ok(());
        };
        let (model, command) = match &setting.provider {
            Provider::Jev { model } => (Some(model), None),
            Provider::Command { command } => (None, Some(command)),
        };
        self.conn
            .prepare_cached(
                "INSERT OR REPLACE INTO judges
                     (project_id, provider, model, command, threshold, ordering, key)
                 SELECT id, ?2, ?3, ?4, ?5, ?6, ?7 FROM projects WHERE name = ?1",
            )
            .map_err(StoreError::new)?
            .execute(params![
                project.as_str(),
                setting.provider.name(),
                model,
                command,
                setting.threshold,
                setting.order.as_str(),
                setting.key(),
            ])
            .map_err(StoreError::new)?;
        Ok(())
    }

    fn verdicts(
        &self,
        id: &ItemId,
        revision: u32,
        key: &str,
    ) -> Result<Option<Vec<Verdict>>, StoreError> {
        let params = params![id.project().as_str(), id.number() as i64, revision, key];
        let judged = self
            .conn
            .prepare_cached(&format!(
                "SELECT 1 FROM judged
                 WHERE item_id = ({ITEM}) AND revision = ?3 AND judged_by = ?4"
            ))
            .map_err(StoreError::new)?
            .exists(params)
            .map_err(StoreError::new)?;
        if !judged {
            return Ok(None);
        }
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT path, importance, reason, promote FROM judged_files
                 WHERE item_id = ({ITEM}) AND revision = ?3 ORDER BY path"
            ))
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(
                params![id.project().as_str(), id.number() as i64, revision],
                |row| {
                    Ok(Verdict {
                        path: row.get(0)?,
                        importance: row.get(1)?,
                        reason: row.get(2)?,
                        promote: row.get(3)?,
                    })
                },
            )
            .map_err(StoreError::new)?;
        rows.collect::<Result<_, _>>()
            .map(Some)
            .map_err(StoreError::new)
    }

    fn insert_verdicts(
        &mut self,
        id: &ItemId,
        revision: u32,
        key: &str,
        verdicts: &[Verdict],
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let item: i64 = tx
            .query_row(
                ITEM,
                params![id.project().as_str(), id.number() as i64],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        tx.execute(
            "DELETE FROM judged WHERE item_id = ?1 AND revision = ?2",
            params![item, revision],
        )
        .map_err(StoreError::new)?;
        tx.execute(
            "INSERT INTO judged (item_id, revision, judged_by) VALUES (?1, ?2, ?3)",
            params![item, revision, key],
        )
        .map_err(StoreError::new)?;
        {
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO judged_files
                         (item_id, revision, path, importance, reason, promote)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(StoreError::new)?;
            for v in verdicts {
                stmt.execute(params![
                    item,
                    revision,
                    v.path,
                    v.importance,
                    v.reason,
                    v.promote
                ])
                .map_err(StoreError::new)?;
            }
        }
        tx.commit().map_err(StoreError::new)
    }
}

#[cfg(test)]
#[path = "judges_tests.rs"]
mod tests;
