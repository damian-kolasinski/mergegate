//! The `item_agents` table.

use mergegate_core::agent::{Agent, AgentStore, DueWake};
use mergegate_core::item::ItemId;
use mergegate_core::project::ProjectName;
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, params};

use crate::Store;

const ITEM: &str = "SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                    WHERE p.name = ?1 AND i.number = ?2";

impl AgentStore for Store {
    fn agent(&self, id: &ItemId) -> Result<Agent, StoreError> {
        let found = self
            .conn
            .query_row(
                &format!("SELECT worktree, wake FROM item_agents WHERE item_id = ({ITEM})"),
                params![id.project().as_str(), id.number() as i64],
                |row| {
                    Ok(Agent {
                        worktree: row.get(0)?,
                        wake: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::new)?;
        Ok(found.unwrap_or_default())
    }

    fn set_agent(&mut self, id: &ItemId, agent: &Agent) -> Result<(), StoreError> {
        self.conn
            .execute(
                &format!(
                    "INSERT INTO item_agents (item_id, worktree, wake) VALUES (({ITEM}), ?3, ?4)
                     ON CONFLICT (item_id) DO UPDATE
                     SET worktree = excluded.worktree, wake = excluded.wake"
                ),
                params![
                    id.project().as_str(),
                    id.number() as i64,
                    agent.worktree,
                    agent.wake
                ],
            )
            .map_err(StoreError::new)?;
        Ok(())
    }

    fn due_wakes(&self) -> Result<Vec<DueWake>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT p.name, i.number, a.wake, a.worktree, e.cursor
                 FROM item_agents a
                 JOIN items i ON i.id = a.item_id
                 JOIN projects p ON p.id = i.project_id
                 JOIN (SELECT item_id, max(cursor) AS cursor FROM events
                       WHERE source <> 'agent' GROUP BY item_id) e
                     ON e.item_id = a.item_id
                 WHERE a.wake IS NOT NULL AND e.cursor > max(i.delivered, a.woken)
                 ORDER BY a.item_id",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(StoreError::new)?;
        rows.map(|row| {
            let (project, number, command, worktree, cursor) = row.map_err(StoreError::new)?;
            let project = ProjectName::parse(&project).map_err(StoreError::new)?;
            Ok(DueWake {
                item: ItemId::new(project, number as u64),
                command,
                worktree,
                cursor: cursor as u64,
            })
        })
        .collect()
    }

    fn set_woken(&mut self, id: &ItemId, cursor: u64) -> Result<(), StoreError> {
        self.conn
            .execute(
                &format!("UPDATE item_agents SET woken = ?3 WHERE item_id = ({ITEM})"),
                params![id.project().as_str(), id.number() as i64, cursor as i64],
            )
            .map_err(StoreError::new)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
