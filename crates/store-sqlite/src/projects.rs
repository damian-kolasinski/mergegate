//! The `projects` table.

use std::path::{Path, PathBuf};

use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, Row, params};

use crate::Store;

const COLUMNS: &str = "name, backend, target, upstream";

impl ProjectStore for Store {
    fn projects(&self) -> Result<Vec<Project>, StoreError> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {COLUMNS} FROM projects ORDER BY name"))
            .map_err(StoreError::new)?;
        let rows = stmt.query_map([], read).map_err(StoreError::new)?;
        rows.map(|row| row.map_err(StoreError::new)?).collect()
    }

    fn project(&self, name: &ProjectName) -> Result<Option<Project>, StoreError> {
        self.project_where("name", name.as_str())
    }

    fn project_with_upstream(&self, upstream: &Path) -> Result<Option<Project>, StoreError> {
        self.project_where("upstream", utf8(upstream)?)
    }

    fn insert_project(&mut self, project: &Project) -> Result<(), StoreError> {
        self.conn
            .execute(
                &format!("INSERT INTO projects ({COLUMNS}) VALUES (?1, ?2, ?3, ?4)"),
                params![
                    project.name.as_str(),
                    project.backend.as_str(),
                    project.target,
                    utf8(&project.upstream)?,
                ],
            )
            .map_err(StoreError::new)?;
        Ok(())
    }

    fn update_project(&mut self, project: &Project) -> Result<(), StoreError> {
        let changed = self
            .conn
            .execute(
                "UPDATE projects SET backend = ?2, target = ?3, upstream = ?4 WHERE name = ?1",
                params![
                    project.name.as_str(),
                    project.backend.as_str(),
                    project.target,
                    utf8(&project.upstream)?,
                ],
            )
            .map_err(StoreError::new)?;
        if changed == 0 {
            return Err(StoreError::new(format!(
                "no project named {}",
                project.name
            )));
        }
        Ok(())
    }
}

impl Store {
    /// `column` is one of the table's unique columns, never caller input.
    fn project_where(&self, column: &str, value: &str) -> Result<Option<Project>, StoreError> {
        self.conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM projects WHERE {column} = ?1"),
                [value],
                read,
            )
            .optional()
            .map_err(StoreError::new)?
            .transpose()
    }
}

/// A row, or the reason it doesn't hold a valid project. The SQL error and
/// the row error are kept apart so a corrupt row reports what is wrong with
/// it rather than a generic conversion failure.
fn read(row: &Row) -> rusqlite::Result<Result<Project, StoreError>> {
    let name: String = row.get(0)?;
    let backend: String = row.get(1)?;
    let target: String = row.get(2)?;
    let upstream: String = row.get(3)?;
    Ok((|| {
        let backend = Backend::parse(&backend)
            .ok_or_else(|| StoreError::new(format!("unknown backend {backend:?}")))?;
        Ok(Project {
            name: ProjectName::parse(&name).map_err(StoreError::new)?,
            backend,
            target,
            upstream: PathBuf::from(upstream),
        })
    })())
}

/// Paths are stored as text, so one that isn't UTF-8 can't be registered.
fn utf8(path: &Path) -> Result<&str, StoreError> {
    path.to_str()
        .ok_or_else(|| StoreError::new(format!("{} is not valid UTF-8", path.display())))
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;
