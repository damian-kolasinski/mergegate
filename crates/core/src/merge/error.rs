use crate::item::{ItemError, ItemId};
use crate::project::{Backend, ProjectError};
use crate::review::ReviewError;
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("no item {0}")]
    NoItem(ItemId),
    #[error("{0} is closed")]
    Closed(ItemId),
    #[error("{0} is already merging")]
    Merging(ItemId),
    #[error("{0} has no merge to retry or cancel")]
    NotMerging(ItemId),
    #[error("{id} has no revision {revision}")]
    NoRevision { id: ItemId, revision: u32 },
    #[error("{id} r{revision} isn't its newest revision; r{newest} is")]
    Stale {
        id: ItemId,
        revision: u32,
        newest: u32,
    },
    #[error("{id} merges through {}, which can't merge yet", .backend.as_str())]
    NoBackend { id: ItemId, backend: Backend },
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Item(#[from] ItemError),
    /// The review approving it can't be sent, as when a draft can't move.
    #[error(transparent)]
    Review(#[from] ReviewError),
    #[error(transparent)]
    Store(#[from] StoreError),
}
