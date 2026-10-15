use super::CommentId;
use crate::git::GitError;
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
    #[error("approving merges the item; approve it instead")]
    Approval,
    #[error("a review needs a summary, a comment or a reply")]
    Empty,
    /// Drafts from earlier revisions whose lines changed: each has to be
    /// re-placed on the review's revision or deleted first (F13).
    #[error("{} draft(s) can't move to {id} r{revision}; re-place or delete them", drafts.len())]
    Unplaced {
        id: ItemId,
        revision: u32,
        drafts: Vec<CommentId>,
    },
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Store(#[from] StoreError),
}
