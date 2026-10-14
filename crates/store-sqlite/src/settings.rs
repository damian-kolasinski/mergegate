//! Hub settings, as key-value rows of `hub_meta`.

use mergegate_core::settings::SettingsStore;
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, params};

use crate::Store;

impl SettingsStore for Store {
    fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.conn
            .prepare_cached("SELECT value FROM hub_meta WHERE key = ?1")
            .map_err(StoreError::new)?
            .query_row([key], |row| row.get(0))
            .optional()
            .map_err(StoreError::new)
    }

    fn set_setting(&mut self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn
            .prepare_cached(
                "INSERT INTO hub_meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            )
            .map_err(StoreError::new)?
            .execute(params![key, value])
            .map_err(StoreError::new)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
