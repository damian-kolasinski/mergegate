//! Schema migrations. The schema version is SQLite's `user_version`.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::StoreError;

/// Migration `n` (1-based) moves the schema from version `n - 1` to `n`. A
/// merged migration is never edited, only followed by a new one.
pub(crate) const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_projects.sql"),
    include_str!("../migrations/0003_items.sql"),
    include_str!("../migrations/0004_diffs.sql"),
    include_str!("../migrations/0005_viewed.sql"),
    include_str!("../migrations/0006_reviews.sql"),
    include_str!("../migrations/0007_comments.sql"),
];

/// The schema version this build migrates databases to.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

pub(crate) fn migrate(
    conn: &mut Connection,
    path: &Path,
    migrations: &[&str],
) -> Result<(), StoreError> {
    let current = user_version(conn)?;
    let latest = migrations.len() as u32;
    if current > latest {
        return Err(StoreError::SchemaTooNew {
            found: current,
            supported: latest,
        });
    }
    if current > 0 && current < latest {
        backup(conn, path, current)?;
    }
    for (version, sql) in (1..).zip(migrations).skip(current as usize) {
        let step = |conn: &mut Connection| -> rusqlite::Result<()> {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version)?;
            tx.commit()
        };
        step(conn).map_err(|source| StoreError::Migration { version, source })?;
    }
    Ok(())
}

pub(crate) fn user_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

/// Where the backup taken before migrating away from `version` is written.
pub fn backup_path(db: &Path, version: u32) -> PathBuf {
    let mut name = db.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".bak-{version}"));
    db.with_file_name(name)
}

/// Writes a consistent copy of the database with `VACUUM INTO`, to a temporary
/// name first so a half-written backup never replaces a good one.
fn backup(conn: &Connection, db: &Path, version: u32) -> Result<(), StoreError> {
    let target = backup_path(db, version);
    let fail = |source: Box<dyn std::error::Error + Send + Sync>| StoreError::Backup {
        path: target.clone(),
        source,
    };
    let mut tmp = target.clone().into_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let tmp_str = tmp
        .to_str()
        .ok_or_else(|| fail("path is not UTF-8".into()))?;
    match std::fs::remove_file(&tmp) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(fail(e.into())),
        _ => {}
    }
    conn.execute("VACUUM INTO ?1", [tmp_str])
        .map_err(|e| fail(e.into()))?;
    std::fs::rename(&tmp, &target).map_err(|e| fail(e.into()))
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
