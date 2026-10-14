//! SQLite store: one database file per hub, in WAL mode, migrated forward on
//! open.

mod agents;
mod attachments;
mod comments;
mod diffs;
mod external;
mod feedback;
mod highlights;
mod items;
mod migrate;
mod projects;
mod replies;
mod reviews;
mod rules;
mod settings;
mod viewed;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

pub use migrate::{SCHEMA_VERSION, backup_path};

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
    /// Attachment files, named by their SHA-256: `attachments/` next to the
    /// database, made on the first attachment.
    attachments: PathBuf,
}

impl Store {
    /// Opens (or creates) the database at `path` and migrates it to
    /// [`SCHEMA_VERSION`], backing up an existing database first (see
    /// [`backup_path`]).
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::open_with(path, migrate::MIGRATIONS)
    }

    fn open_with(path: &Path, migrations: &[&str]) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate::migrate(&mut conn, path, migrations)?;
        Ok(Self {
            conn,
            attachments: path.with_file_name("attachments"),
        })
    }

    pub fn schema_version(&self) -> Result<u32, StoreError> {
        Ok(migrate::user_version(&self.conn)?)
    }
}
