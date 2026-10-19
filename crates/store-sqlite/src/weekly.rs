use std::collections::BTreeMap;

use mergegate_core::item::{RepositoryClosures, WeeklyClosures};
use mergegate_core::project::ProjectName;
use mergegate_core::store::StoreError;
use rusqlite::params;

use crate::Store;

impl Store {
    pub(crate) fn weekly_closures_at(&self, now: &str) -> Result<WeeklyClosures, StoreError> {
        let (monday, today): (String, String) = self
            .conn
            .query_row(
                "SELECT date(?1, 'localtime', 'weekday 0', '-6 days'),
                        date(?1, 'localtime')",
                [now],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(StoreError::new)?;
        // Convert the local midnights separately: a DST week need not be
        // seven times 24 hours, and a closure belongs to its local day.
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT p.name,
                        (CAST(strftime('%w', i.closed_at, 'localtime') AS INTEGER) + 6) % 7,
                        count(*)
                 FROM items i JOIN projects p ON p.id = i.project_id
                 WHERE i.state IN ('merged', 'abandoned')
                   AND i.closed_at >= strftime('%Y-%m-%dT%H:%M:%fZ', ?1, 'utc')
                   AND i.closed_at < strftime('%Y-%m-%dT%H:%M:%fZ', ?2, '+1 day', 'utc')
                 GROUP BY p.name, 2",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(params![monday, today], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u8>(1)?,
                    row.get::<_, u32>(2)?,
                ))
            })
            .map_err(StoreError::new)?;
        let mut repositories = BTreeMap::new();
        for row in rows {
            let (project, day, count) = row.map_err(StoreError::new)?;
            repositories.entry(project).or_insert([0; 7])[usize::from(day)] = count;
        }
        let repositories = repositories
            .into_iter()
            .map(|(project, days)| {
                Ok(RepositoryClosures {
                    project: ProjectName::parse(&project).map_err(StoreError::new)?,
                    days,
                })
            })
            .collect::<Result<_, StoreError>>()?;
        Ok(WeeklyClosures {
            monday,
            today,
            repositories,
        })
    }
}

#[cfg(test)]
#[path = "weekly_tests.rs"]
mod tests;
