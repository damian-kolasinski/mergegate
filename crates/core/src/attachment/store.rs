use std::time::Duration;

use super::{Attachment, Media};
use crate::item::ItemId;
use crate::store::StoreError;

pub trait AttachmentStore {
    /// The item's attachments, by revision, then in the order they were
    /// added.
    fn attachments(&self, id: &ItemId) -> Result<Vec<Attachment>, StoreError>;

    /// Keeps `attachment.bytes`, once however many attachments share them,
    /// and adds them to the end of the revision's attachments.
    fn add_attachment(
        &mut self,
        id: &ItemId,
        attachment: NewAttachment<'_>,
    ) -> Result<Attachment, StoreError>;

    /// The type of the file with this hash, while an attachment uses it.
    fn attachment_media(&self, sha256: &str) -> Result<Option<Media>, StoreError>;

    /// Drops the attachments of items that closed at least `ttl` ago, then
    /// every kept file no attachment uses. Returns the items that lost any.
    fn prune_attachments(&mut self, ttl: Duration) -> Result<Vec<ItemId>, StoreError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewAttachment<'a> {
    pub revision: u32,
    pub name: &'a str,
    pub media: Media,
    pub bytes: &'a [u8],
}
