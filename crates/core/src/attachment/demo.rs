use super::{Attachment, AttachmentStore};
use crate::item::ItemId;
use crate::store::StoreError;

/// What the review screen's demo panel shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Demo {
    /// The revision they were attached to, which may be older than the
    /// newest.
    pub revision: u32,
    /// In the order they were added.
    pub attachments: Vec<Attachment>,
}

/// The item's demo; see [`choose`].
pub fn demo<S: AttachmentStore>(store: &S, id: &ItemId) -> Result<Option<Demo>, StoreError> {
    Ok(choose(store.attachments(id)?))
}

/// The newest revision's attachments, or when it has none the newest
/// earlier revision's, so a small fix needn't record its demo again.
pub fn choose(attachments: Vec<Attachment>) -> Option<Demo> {
    let revision = attachments.iter().map(|a| a.revision).max()?;
    Some(Demo {
        revision,
        attachments: attachments
            .into_iter()
            .filter(|a| a.revision == revision)
            .collect(),
    })
}

#[cfg(test)]
#[path = "demo_tests.rs"]
mod tests;
