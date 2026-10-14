use super::MAX_SIZE;
use crate::item::ItemId;
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum AttachmentError {
    #[error("no item {0}")]
    NoItem(ItemId),
    #[error("{0} is closed; attach to an open item")]
    Closed(ItemId),
    #[error("{id} has no revision {revision}")]
    NoRevision { id: ItemId, revision: u32 },
    #[error("{0:?} is not a file name")]
    InvalidName(String),
    #[error("{name} is {size} bytes, over the {} MB limit", MAX_SIZE / 1024 / 1024)]
    TooLarge { name: String, size: u64 },
    #[error("{0} is not a PNG, JPEG, GIF or WebP image, or an MP4 or WebM video")]
    NotMedia(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}
