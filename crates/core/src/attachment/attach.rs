use super::{Attachment, AttachmentError, AttachmentStore, MAX_SIZE, Media, NewAttachment};
use crate::item::{ItemId, ItemStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attach<'a> {
    pub id: &'a ItemId,
    /// None attaches to the newest.
    pub revision: Option<u32>,
    pub name: &'a str,
    pub bytes: &'a [u8],
}

/// Adds a screenshot or recording to one of an open item's revisions.
pub fn attach<S: ItemStore + AttachmentStore>(
    store: &mut S,
    request: Attach<'_>,
) -> Result<Attachment, AttachmentError> {
    let Attach {
        id,
        revision,
        name,
        bytes,
    } = request;
    let item = store
        .item(id)?
        .ok_or_else(|| AttachmentError::NoItem(id.clone()))?;
    if !item.state.is_open() {
        return Err(AttachmentError::Closed(item.id));
    }
    let revision = revision.unwrap_or(item.revision.number);
    if revision == 0 || revision > item.revision.number {
        return Err(AttachmentError::NoRevision {
            id: item.id,
            revision,
        });
    }
    if !super::valid_name(name) {
        return Err(AttachmentError::InvalidName(name.into()));
    }
    let size = bytes.len() as u64;
    if size > MAX_SIZE {
        return Err(AttachmentError::TooLarge {
            name: name.into(),
            size,
        });
    }
    let media = Media::sniff(bytes).ok_or_else(|| AttachmentError::NotMedia(name.into()))?;
    let attachment = NewAttachment {
        revision,
        name,
        media,
        bytes,
    };
    Ok(store.add_attachment(id, attachment)?)
}

#[cfg(test)]
#[path = "attach_tests.rs"]
mod tests;
