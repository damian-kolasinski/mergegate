use super::AttachmentStore;
use crate::item::ItemId;
use crate::settings::{self, SettingsError, SettingsStore};

/// Drops what closed items attached once they've been closed for the hub's
/// `attachment-ttl`. Returns the items that lost any.
pub fn prune<S: AttachmentStore + SettingsStore>(
    store: &mut S,
) -> Result<Vec<ItemId>, SettingsError> {
    let ttl = settings::settings(store)?.attachment_ttl;
    Ok(store.prune_attachments(ttl.duration())?)
}
