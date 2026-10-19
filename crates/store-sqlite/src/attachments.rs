//! The `attachments` table, and the files it names: one per content, under
//! the attachments folder, named by their SHA-256.

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;

use mergegate_core::attachment::{Attachment, AttachmentStore, Media, NewAttachment};
use mergegate_core::item::ItemId;
use mergegate_core::project::ProjectName;
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::Store;
use crate::items::NOW;

/// `?1` and `?2` name the item.
const ITEM: &str = "SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                    WHERE p.name = ?1 AND i.number = ?2";

impl Store {
    /// Where the file with this hash is kept, if an attachment uses it.
    pub fn attachment_file(&self, sha256: &str) -> PathBuf {
        self.attachments.join(sha256)
    }

    /// Writes `bytes` under its hash unless a file is there already. Through
    /// a temporary name, so a crash never leaves a short file under a hash.
    fn keep(&self, sha256: &str, bytes: &[u8]) -> Result<(), StoreError> {
        let path = self.attachment_file(sha256);
        if path.exists() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.attachments).map_err(StoreError::new)?;
        let tmp = self.attachments.join(format!("{sha256}.tmp"));
        std::fs::write(&tmp, bytes).map_err(StoreError::new)?;
        std::fs::rename(&tmp, &path).map_err(StoreError::new)
    }

    /// Removes every file no attachment names, a crashed write's included.
    fn sweep_files(&self) -> Result<(), StoreError> {
        let entries = match std::fs::read_dir(&self.attachments) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(StoreError::new(e)),
        };
        let mut stmt = self
            .conn
            .prepare_cached("SELECT DISTINCT sha256 FROM attachments")
            .map_err(StoreError::new)?;
        let used: HashSet<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(StoreError::new)?
            .collect::<Result<_, _>>()
            .map_err(StoreError::new)?;
        for entry in entries {
            let entry = entry.map_err(StoreError::new)?;
            let name = entry.file_name();
            if name.to_str().is_some_and(|name| used.contains(name)) {
                continue;
            }
            match std::fs::remove_file(entry.path()) {
                Err(e) if e.kind() != ErrorKind::NotFound => return Err(StoreError::new(e)),
                _ => {}
            }
        }
        Ok(())
    }
}

impl AttachmentStore for Store {
    fn attachments(&self, id: &ItemId) -> Result<Vec<Attachment>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT revision, name, media, size, sha256, crc32 FROM attachments
                 WHERE item_id = ({ITEM}) ORDER BY revision, position"
            ))
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(params![id.project().as_str(), id.number() as i64], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<u32>>(5)?,
                ))
            })
            .map_err(StoreError::new)?;
        rows.map(|row| {
            let (revision, name, media, size, sha256, crc32) = row.map_err(StoreError::new)?;
            Ok(Attachment {
                revision,
                name,
                media: media_of(&media)?,
                size: size as u64,
                sha256,
                crc32,
            })
        })
        .collect()
    }

    fn add_attachment(
        &mut self,
        id: &ItemId,
        attachment: NewAttachment<'_>,
    ) -> Result<Attachment, StoreError> {
        let sha256: String = Sha256::digest(attachment.bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let crc32 = crc32fast::hash(attachment.bytes);
        // The file goes first: a row naming a missing file would break the
        // panel, while a file with no row is swept by the next prune.
        self.keep(&sha256, attachment.bytes)?;
        let size = attachment.bytes.len() as u64;
        let item_id: i64 = self
            .conn
            .query_row(
                ITEM,
                params![id.project().as_str(), id.number() as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::new)?
            .ok_or_else(|| StoreError::new(format!("no item {id}")))?;
        self.conn
            .prepare_cached(&format!(
                "INSERT INTO attachments
                     (item_id, revision, position, name, media, size, sha256, crc32,
                      created_at)
                 VALUES (?1, ?2, coalesce(
                             (SELECT max(position) + 1 FROM attachments
                              WHERE item_id = ?1 AND revision = ?2), 0),
                         ?3, ?4, ?5, ?6, ?7, {NOW})"
            ))
            .map_err(StoreError::new)?
            .execute(params![
                item_id,
                attachment.revision,
                attachment.name,
                attachment.media.mime(),
                size as i64,
                sha256,
                crc32
            ])
            .map_err(StoreError::new)?;
        Ok(Attachment {
            revision: attachment.revision,
            name: attachment.name.into(),
            media: attachment.media,
            size,
            sha256,
            crc32: Some(crc32),
        })
    }

    fn attachment_media(&self, sha256: &str) -> Result<Option<Media>, StoreError> {
        let media: Option<String> = self
            .conn
            .prepare_cached("SELECT media FROM attachments WHERE sha256 = ?1 LIMIT 1")
            .map_err(StoreError::new)?
            .query_row([sha256], |row| row.get(0))
            .optional()
            .map_err(StoreError::new)?;
        media.as_deref().map(media_of).transpose()
    }

    fn prune_attachments(&mut self, ttl: Duration) -> Result<Vec<ItemId>, StoreError> {
        let modifier = format!("-{} seconds", ttl.as_secs());
        // One cutoff for both statements, so the items listed are the ones
        // whose attachments go.
        let cutoff: String = self
            .conn
            .query_row(
                "SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1)",
                [modifier],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        let items = self
            .conn
            .prepare_cached(
                "SELECT p.name, i.number FROM items i JOIN projects p ON p.id = i.project_id
                 WHERE i.closed_at <= ?1
                 AND EXISTS (SELECT 1 FROM attachments a WHERE a.item_id = i.id)",
            )
            .map_err(StoreError::new)?
            .query_map([&cutoff], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(StoreError::new)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::new)?;
        self.conn
            .prepare_cached(
                "DELETE FROM attachments
                 WHERE item_id IN (SELECT id FROM items WHERE closed_at <= ?1)",
            )
            .map_err(StoreError::new)?
            .execute([&cutoff])
            .map_err(StoreError::new)?;
        self.sweep_files()?;
        items
            .into_iter()
            .map(|(project, number)| {
                let project = ProjectName::parse(&project).map_err(StoreError::new)?;
                Ok(ItemId::new(project, number as u64))
            })
            .collect()
    }
}

fn media_of(mime: &str) -> Result<Media, StoreError> {
    Media::from_mime(mime).ok_or_else(|| StoreError::new(format!("unknown media {mime:?}")))
}

#[cfg(test)]
#[path = "attachments_tests.rs"]
mod tests;
