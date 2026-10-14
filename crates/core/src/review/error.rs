use super::{CommentId, Side};
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
    #[error("a review needs a summary or a comment")]
    Empty,
    #[error("a comment needs a body")]
    EmptyComment,
    #[error("{id} r{revision} doesn't change {path}")]
    NoFile {
        id: ItemId,
        revision: u32,
        path: String,
    },
    #[error("{path} has no lines {start_line}-{line} on the {} side", side.as_str())]
    NoLines {
        path: String,
        side: Side,
        start_line: u32,
        line: u32,
    },
    #[error("{0} isn't text, so it has no lines to comment on")]
    NotText(String),
    #[error("{id} has no draft {comment}")]
    NoDraft { id: ItemId, comment: CommentId },
    #[error("{0} was sent, so it can't change")]
    Sent(CommentId),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Store(#[from] StoreError),
}
