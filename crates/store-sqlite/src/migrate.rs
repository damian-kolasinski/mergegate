//! Schema migrations. The schema version is SQLite's `user_version`.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use crate::StoreError;

/// Migration `n` (1-based) moves the schema from version `n - 1` to `n`. A
/// merged migration is never edited, only followed by a new one.
pub(crate) const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_external_review.sql"),
    include_str!("../migrations/0003_highlights.sql"),
    include_str!("../migrations/0004_attachments.sql"),
    include_str!("../migrations/0005_triage.sql"),
    include_str!("../migrations/0006_rule_changes.sql"),
    include_str!("../migrations/0007_guides.sql"),
    include_str!("../migrations/0008_interdiffs.sql"),
    include_str!("../migrations/0009_priority.sql"),
    include_str!("../migrations/0010_conflicts.sql"),
    include_str!("../migrations/0011_devices.sql"),
    include_str!("../migrations/0012_judge.sql"),
    include_str!("../migrations/0013_file_modes.sql"),
    include_str!("../migrations/0014_external_dated.sql"),
    include_str!("../migrations/0015_interdiff_added.sql"),
    include_str!("../migrations/0016_diff_lines_after.sql"),
    include_str!("../migrations/0017_closed_on_github.sql"),
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
    // A migration may rebuild a table others reference, which SQLite does
    // with foreign keys off (its "other kinds of table schema changes"), and
    // the pragma does nothing inside a transaction. Each step checks the keys
    // itself before it commits.
    conn.pragma_update(None, "foreign_keys", false)?;
    let migrated = (1..)
        .zip(migrations)
        .skip(current as usize)
        .try_for_each(|(version, sql)| {
            let step = |conn: &mut Connection| -> rusqlite::Result<()> {
                let tx = conn.transaction()?;
                tx.execute_batch(sql)?;
                let broken: Option<String> = tx
                    .query_row("PRAGMA foreign_key_check", [], |row| row.get(0))
                    .optional()?;
                if let Some(table) = broken {
                    return Err(rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
                        Some(format!("a row of {table} references one that's gone")),
                    ));
                }
                tx.pragma_update(None, "user_version", version)?;
                tx.commit()
            };
            step(conn).map_err(|source| StoreError::Migration { version, source })
        });
    conn.pragma_update(None, "foreign_keys", true)?;
    migrated
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
