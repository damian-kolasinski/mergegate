//! Tailnet devices and the operator's decisions about them.

use mergegate_core::device::{Access, DeviceId, DeviceStore, Record};
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, Row, params};

use crate::Store;

const COLUMNS: &str = "id, name, owner, access, asked_at";

impl DeviceStore for Store {
    fn device(&self, id: &DeviceId) -> Result<Option<Record>, StoreError> {
        self.conn
            .prepare_cached(&format!("SELECT {COLUMNS} FROM devices WHERE id = ?1"))
            .map_err(StoreError::new)?
            .query_row([&id.0], record)
            .optional()
            .map_err(StoreError::new)
    }

    fn devices(&self) -> Result<Vec<Record>, StoreError> {
        self.conn
            .prepare_cached(&format!("SELECT {COLUMNS} FROM devices ORDER BY name, id"))
            .map_err(StoreError::new)?
            .query_map([], record)
            .map_err(StoreError::new)?
            .collect::<Result<_, _>>()
            .map_err(StoreError::new)
    }

    fn put_device(&mut self, record: &Record) -> Result<(), StoreError> {
        let access = record.access.map(|access| match access {
            Access::Allow => "allow",
            Access::Deny => "deny",
        });
        let asked_at = record.asked_at.map(|secs| secs as i64);
        self.conn
            .prepare_cached(
                "INSERT INTO devices (id, name, owner, access, asked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (id) DO UPDATE SET
                     name = excluded.name, owner = excluded.owner,
                     access = excluded.access, asked_at = excluded.asked_at",
            )
            .map_err(StoreError::new)?
            .execute(params![
                record.id.0,
                record.name,
                record.owner,
                access,
                asked_at
            ])
            .map_err(StoreError::new)?;
        Ok(())
    }
}

fn record(row: &Row) -> rusqlite::Result<Record> {
    let access: Option<String> = row.get(3)?;
    let asked_at: Option<i64> = row.get(4)?;
    Ok(Record {
        id: DeviceId(row.get(0)?),
        name: row.get(1)?,
        owner: row.get(2)?,
        // The CHECK constraint keeps anything else out.
        access: access.map(|a| match a.as_str() {
            "deny" => Access::Deny,
            _ => Access::Allow,
        }),
        asked_at: asked_at.map(|secs| secs as u64),
    })
}

#[cfg(test)]
#[path = "devices_tests.rs"]
mod tests;
