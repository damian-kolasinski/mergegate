//! SQLite store: one database file per hub, in WAL mode, migrated forward on
//! open. The schema version is SQLite's `user_version`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

/// Migrations in order; migration `n` (1-based) moves the schema from version
/// `n - 1` to `n`. A merged migration is never edited, only followed by a new one.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

/// The schema version this build migrates databases to.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("the database is at schema {found}, newer than this mg supports ({supported})")]
    SchemaTooNew { found: u32, supported: u32 },
    #[error("backing up the database to {path} before migrating failed: {source}")]
    Backup {
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("migrating the database to schema {version} failed: {source}")]
    Migration {
        version: u32,
        source: rusqlite::Error,
    },
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens (or creates) the database at `path` and migrates it to
    /// [`SCHEMA_VERSION`]. Before migrating an existing database it writes a
    /// copy next to it as `<file name>.bak-<current schema version>`.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        open_with(path, MIGRATIONS)
    }

    pub fn schema_version(&self) -> Result<u32, StoreError> {
        Ok(user_version(&self.conn)?)
    }
}

/// Where the backup taken before migrating away from `version` is written.
pub fn backup_path(db: &Path, version: u32) -> PathBuf {
    let mut name = db.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".bak-{version}"));
    db.with_file_name(name)
}

fn open_with(path: &Path, migrations: &[&str]) -> Result<Store, StoreError> {
    let mut conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
    conn.pragma_update(None, "foreign_keys", true)?;

    let current = user_version(&conn)?;
    let latest = migrations.len() as u32;
    if current > latest {
        return Err(StoreError::SchemaTooNew {
            found: current,
            supported: latest,
        });
    }
    if current > 0 && current < latest {
        backup(&conn, path, current)?;
    }
    for (version, sql) in (1..).zip(migrations).skip(current as usize) {
        let migrate = |conn: &mut Connection| -> rusqlite::Result<()> {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version)?;
            tx.commit()
        };
        migrate(&mut conn).map_err(|source| StoreError::Migration { version, source })?;
    }
    Ok(Store { conn })
}

fn user_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
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
mod tests {
    use super::*;

    fn tables(path: &Path) -> Vec<String> {
        let conn = Connection::open(path).unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn version_of(path: &Path) -> u32 {
        user_version(&Connection::open(path).unwrap()).unwrap()
    }

    #[test]
    fn new_database_is_migrated_without_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("hub.db");
        let store = Store::open(&db).unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        drop(store);
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(!names.iter().any(|n| n.contains(".bak-")), "{names:?}");
    }

    #[test]
    fn upgrades_from_every_earlier_schema_version() {
        for from in 0..SCHEMA_VERSION {
            let dir = tempfile::tempdir().unwrap();
            let db = dir.path().join("hub.db");
            drop(open_with(&db, &MIGRATIONS[..from as usize]).unwrap());
            let store = Store::open(&db).unwrap();
            assert_eq!(
                store.schema_version().unwrap(),
                SCHEMA_VERSION,
                "from {from}"
            );
            if from > 0 {
                assert_eq!(
                    version_of(&backup_path(&db, from)),
                    from,
                    "backup from {from}"
                );
            }
        }
    }

    #[test]
    fn backs_up_before_migrating_an_existing_database() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("hub.db");
        drop(open_with(&db, &["CREATE TABLE a (x)"]).unwrap());
        drop(open_with(&db, &["CREATE TABLE a (x)", "CREATE TABLE b (y)"]).unwrap());

        assert_eq!(version_of(&db), 2);
        assert_eq!(tables(&db), ["a", "b"]);
        let backup = backup_path(&db, 1);
        assert_eq!(backup.file_name().unwrap(), "hub.db.bak-1");
        assert_eq!(version_of(&backup), 1);
        assert_eq!(tables(&backup), ["a"]);
    }

    #[test]
    fn replaces_an_earlier_backup_of_the_same_version() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("hub.db");
        std::fs::write(backup_path(&db, 1), "stale").unwrap();
        drop(open_with(&db, &["CREATE TABLE a (x)"]).unwrap());
        drop(open_with(&db, &["CREATE TABLE a (x)", "CREATE TABLE b (y)"]).unwrap());
        assert_eq!(tables(&backup_path(&db, 1)), ["a"]);
    }

    #[test]
    fn failed_migration_leaves_the_schema_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("hub.db");
        drop(open_with(&db, &["CREATE TABLE a (x)"]).unwrap());
        let err = open_with(&db, &["CREATE TABLE a (x)", "CREATE TABLE b (y); NOT SQL"])
            .err()
            .unwrap();
        assert!(
            matches!(err, StoreError::Migration { version: 2, .. }),
            "{err}"
        );
        assert_eq!(version_of(&db), 1);
        assert_eq!(tables(&db), ["a"]);
    }

    #[test]
    fn refuses_a_database_newer_than_this_build() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("hub.db");
        drop(open_with(&db, &["CREATE TABLE a (x)", "CREATE TABLE b (y)"]).unwrap());
        let err = open_with(&db, &["CREATE TABLE a (x)"]).err().unwrap();
        assert!(matches!(
            err,
            StoreError::SchemaTooNew {
                found: 2,
                supported: 1
            }
        ));
    }
}
