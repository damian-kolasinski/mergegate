//! The `items`, `revisions` and `revision_files` tables.

use mergegate_core::github::{GithubRepo, Pull};
use mergegate_core::item::{
    ChangedFile, DiffStat, FileStat, Item, ItemBackend, ItemId, ItemState, ItemStore, NewItem,
    NewRevision, Revision,
};
use mergegate_core::project::{Backend, ProjectName};
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, Row, Transaction, params};

use crate::Store;
use crate::projects::backend_named;
use crate::rules::{columns, read_triage};

/// An item with its newest revision. Callers add a `WHERE` and `ORDER BY`.
const SELECT: &str = "
    SELECT p.name, i.number, i.branch, i.title, i.state, i.backend, i.target, i.created_at,
           r.number, r.head, r.base, r.files, r.additions, r.deletions, r.submitted_at,
           i.github, i.pull_number, i.pull_url,
           (SELECT count(*) FROM revision_files
            WHERE item_id = i.id AND revision = r.number AND tier = 'review')
    FROM items i
    JOIN projects p ON p.id = i.project_id
    JOIN revisions r ON r.item_id = i.id
        AND r.number = (SELECT max(number) FROM revisions WHERE item_id = i.id)";

/// Matches the `items_open_branch` index.
const OPEN: &str = "i.state IN ('needs_review', 'changes_requested', 'merging')";

pub(crate) const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

impl ItemStore for Store {
    fn open_items(&self, project: Option<&ProjectName>) -> Result<Vec<Item>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "{SELECT} WHERE {OPEN} AND (?1 IS NULL OR p.name = ?1) ORDER BY i.id"
            ))
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map([project.map(ProjectName::as_str)], read)
            .map_err(StoreError::new)?;
        rows.map(|row| row.map_err(StoreError::new)?).collect()
    }

    fn open_item(&self, project: &ProjectName, branch: &str) -> Result<Option<Item>, StoreError> {
        self.conn
            .query_row(
                &format!("{SELECT} WHERE {OPEN} AND p.name = ?1 AND i.branch = ?2"),
                [project.as_str(), branch],
                read,
            )
            .optional()
            .map_err(StoreError::new)?
            .transpose()
    }

    fn item(&self, id: &ItemId) -> Result<Option<Item>, StoreError> {
        self.conn
            .query_row(
                &format!("{SELECT} WHERE p.name = ?1 AND i.number = ?2"),
                params![id.project().as_str(), id.number() as i64],
                read,
            )
            .optional()
            .map_err(StoreError::new)?
            .transpose()
    }

    fn revisions(&self, id: &ItemId) -> Result<Vec<Revision>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT r.number, r.head, r.base, r.files, r.additions, r.deletions, r.submitted_at,
                        (SELECT count(*) FROM revision_files
                         WHERE item_id = r.item_id AND revision = r.number AND tier = 'review')
                 FROM revisions r
                 JOIN items i ON i.id = r.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2
                 ORDER BY r.number",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(params![id.project().as_str(), id.number() as i64], |row| {
                Ok(Revision {
                    number: row.get(0)?,
                    head: row.get(1)?,
                    base: row.get(2)?,
                    stat: DiffStat {
                        files: row.get(3)?,
                        additions: row.get::<_, i64>(4)? as u64,
                        deletions: row.get::<_, i64>(5)? as u64,
                        review_files: row.get(7)?,
                    },
                    submitted_at: row.get(6)?,
                })
            })
            .map_err(StoreError::new)?;
        rows.collect::<Result<_, _>>().map_err(StoreError::new)
    }

    fn revision_files(&self, id: &ItemId, revision: u32) -> Result<Vec<FileStat>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT f.path, f.old_path, f.old_blob, f.new_blob, f.additions, f.deletions,
                        f.generated, f.tier, f.rule, f.guard, f.why
                 FROM revision_files f
                 JOIN items i ON i.id = f.item_id
                 JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2 AND f.revision = ?3
                 ORDER BY f.position",
            )
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(
                params![id.project().as_str(), id.number() as i64, revision],
                |row| {
                    let triage = match read_triage(row, 7)? {
                        Ok(triage) => triage,
                        Err(e) => return Ok(Err(e)),
                    };
                    Ok(Ok(FileStat {
                        file: ChangedFile {
                            path: row.get(0)?,
                            old_path: row.get(1)?,
                            old_blob: row.get(2)?,
                            new_blob: row.get(3)?,
                        },
                        additions: row.get::<_, Option<i64>>(4)?.map(|n| n as u64),
                        deletions: row.get::<_, Option<i64>>(5)?.map(|n| n as u64),
                        generated: row.get(6)?,
                        triage,
                    }))
                },
            )
            .map_err(StoreError::new)?;
        rows.map(|row| row.map_err(StoreError::new)?).collect()
    }

    fn insert_item(&mut self, item: NewItem) -> Result<Item, StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let (project_id, number): (i64, i64) = tx
            .query_row(
                "SELECT p.id, (SELECT coalesce(max(number), 0) + 1 FROM items WHERE project_id = p.id)
                 FROM projects p WHERE p.name = ?1",
                [item.project.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(StoreError::new)?;
        let now: String = tx
            .query_row(&format!("SELECT {NOW}"), [], |row| row.get(0))
            .map_err(StoreError::new)?;
        let (github, pull) = match &item.backend {
            ItemBackend::Github { repo, pull } => (Some(repo.to_string()), Some(pull)),
            ItemBackend::Local => (None, None),
        };
        tx.execute(
            "INSERT INTO items (project_id, number, branch, title, state, backend, target, created_at,
                                github, pull_number, pull_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                project_id,
                number,
                item.branch,
                item.title,
                item.state.as_str(),
                item.backend.kind().as_str(),
                item.target,
                now,
                github,
                pull.map(|pull| pull.number as i64),
                pull.map(|pull| pull.url.as_str()),
            ],
        )
        .map_err(StoreError::new)?;
        insert_revision(&tx, tx.last_insert_rowid(), 1, &item.revision, &now)?;
        tx.commit().map_err(StoreError::new)?;
        let id = ItemId::new(item.project, number as u64);
        self.written(&id)
    }

    fn add_revision(
        &mut self,
        id: &ItemId,
        title: &str,
        state: ItemState,
        revision: NewRevision,
    ) -> Result<Item, StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let (item_id, number): (i64, i64) = tx
            .query_row(
                "SELECT i.id, (SELECT max(number) + 1 FROM revisions WHERE item_id = i.id)
                 FROM items i JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2",
                params![id.project().as_str(), id.number() as i64],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(StoreError::new)?;
        let now: String = tx
            .query_row(&format!("SELECT {NOW}"), [], |row| row.get(0))
            .map_err(StoreError::new)?;
        insert_revision(&tx, item_id, number, &revision, &now)?;
        tx.execute(
            "UPDATE items SET title = ?2, state = ?3 WHERE id = ?1",
            params![item_id, title, state.as_str()],
        )
        .map_err(StoreError::new)?;
        tx.commit().map_err(StoreError::new)?;
        self.written(id)
    }
}

impl Store {
    /// The item a write just committed.
    fn written(&self, id: &ItemId) -> Result<Item, StoreError> {
        self.item(id)?
            .ok_or_else(|| StoreError::new(format!("{id} vanished after it was written")))
    }
}

fn insert_revision(
    tx: &Transaction,
    item_id: i64,
    number: i64,
    revision: &NewRevision,
    now: &str,
) -> Result<(), StoreError> {
    let stat = DiffStat::of(&revision.files);
    tx.execute(
        "INSERT INTO revisions
             (item_id, number, head, base, files, additions, deletions, submitted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            item_id,
            number,
            revision.head,
            revision.base,
            stat.files,
            stat.additions as i64,
            stat.deletions as i64,
            now,
        ],
    )
    .map_err(StoreError::new)?;
    let mut stmt = tx
        .prepare_cached(
            "INSERT INTO revision_files
                 (item_id, revision, position, path, old_path, old_blob, new_blob,
                  additions, deletions, generated, tier, rule, guard, why)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )
        .map_err(StoreError::new)?;
    for (position, file) in revision.files.iter().enumerate() {
        let [tier, rule, guard, why] = columns(&file.triage);
        stmt.execute(params![
            item_id,
            number,
            position as i64,
            file.file.path,
            file.file.old_path,
            file.file.old_blob,
            file.file.new_blob,
            file.additions.map(|n| n as i64),
            file.deletions.map(|n| n as i64),
            file.generated,
            tier,
            rule,
            guard,
            why,
        ])
        .map_err(StoreError::new)?;
    }
    Ok(())
}

/// A row, or the reason it doesn't hold a valid item; see `projects::read`.
fn read(row: &Row) -> rusqlite::Result<Result<Item, StoreError>> {
    let project: String = row.get(0)?;
    let number: i64 = row.get(1)?;
    let state: String = row.get(4)?;
    let backend: String = row.get(5)?;
    let revision = Revision {
        number: row.get(8)?,
        head: row.get(9)?,
        base: row.get(10)?,
        stat: DiffStat {
            files: row.get(11)?,
            additions: row.get::<_, i64>(12)? as u64,
            deletions: row.get::<_, i64>(13)? as u64,
            review_files: row.get(18)?,
        },
        submitted_at: row.get(14)?,
    };
    let (branch, title, target, created_at) = (row.get(2)?, row.get(3)?, row.get(6)?, row.get(7)?);
    let github: Option<String> = row.get(15)?;
    let pull_number: Option<i64> = row.get(16)?;
    let pull_url: Option<String> = row.get(17)?;
    let pull = pull_number.zip(pull_url).map(|(number, url)| Pull {
        number: number as u64,
        url,
    });
    Ok((|| {
        let state = ItemState::ALL
            .into_iter()
            .find(|s| s.as_str() == state)
            .ok_or_else(|| StoreError::new(format!("unknown item state {state:?}")))?;
        let backend = match (backend_named(&backend)?, github, pull) {
            (Backend::Github, Some(repo), Some(pull)) => ItemBackend::Github {
                repo: GithubRepo::parse(&repo)
                    .ok_or_else(|| StoreError::new(format!("invalid GitHub repo {repo:?}")))?,
                pull,
            },
            (Backend::Local, None, None) => ItemBackend::Local,
            (backend, ..) => {
                let backend = backend.as_str();
                return Err(StoreError::new(format!(
                    "a {backend} item whose GitHub repo or pull request doesn't match its backend"
                )));
            }
        };
        let project = ProjectName::parse(&project).map_err(StoreError::new)?;
        Ok(Item {
            id: ItemId::new(project, number as u64),
            branch,
            title,
            state,
            backend,
            target,
            revision,
            created_at,
        })
    })())
}

#[cfg(test)]
#[path = "items_tests.rs"]
mod tests;
