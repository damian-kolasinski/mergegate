//! An item's comments and their replies, on every revision: what a review
//! screen rereads as the agent answers, without its diff.

use super::{Comment, Reply, ReviewStore};
use crate::item::{ItemId, ItemStore};
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    /// The item's newest revision, which tells whether one addressed a reply.
    pub newest: u32,
    /// Drafts and sent ones, oldest first.
    pub comments: Vec<Comment>,
    /// On every comment, the operator's drafts included, oldest first.
    pub replies: Vec<Reply>,
}

/// `None` when there's no such item.
pub fn conversation<S: ItemStore + ReviewStore>(
    store: &S,
    id: &ItemId,
) -> Result<Option<Conversation>, StoreError> {
    let Some(item) = store.item(id)? else {
        return Ok(None);
    };
    read(store, id, item.revision.number).map(Some)
}

/// The conversation of an item the caller has read.
pub(crate) fn read<S: ReviewStore>(
    store: &S,
    id: &ItemId,
    newest: u32,
) -> Result<Conversation, StoreError> {
    Ok(Conversation {
        newest,
        comments: store.comments(id)?,
        replies: store.replies(id)?,
    })
}

#[cfg(test)]
#[path = "conversation_tests.rs"]
mod tests;
