use crate::item::ItemId;
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("no item {0}")]
    NoItem(ItemId),
    #[error("{id} has no revision {revision}")]
    NoRevision { id: ItemId, revision: u32 },
    #[error("{0} is closed")]
    Closed(ItemId),
    #[error("{0} is merging")]
    Merging(ItemId),
    #[error("a review needs a summary, a comment or a reply")]
    Empty,
    #[error(transparent)]
    Store(#[from] StoreError),
}
