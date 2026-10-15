//! An item's comments and their replies, on every revision: what a review
//! screen rereads as the agent answers, without its diff.

use std::collections::BTreeMap;

use super::{Comment, CommentId, Reply, ReviewStore};
use crate::diff::{DiffGit, DiffStore};
use crate::interdiff::InterdiffStore;
use crate::item::{Item, ItemId, ItemStore};
use crate::remap::{PlaceError, Placement, place};
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    /// The item's newest revision, which tells whether one addressed a reply.
    pub newest: u32,
    /// Drafts and sent ones, oldest first.
    pub comments: Vec<Comment>,
    /// On every comment, the operator's drafts included, oldest first.
    pub replies: Vec<Reply>,
    /// Where each comment from a revision before the one on screen is on it.
    /// Empty once the item is closed, and for what agents read: they get
    /// comments where they were written.
    pub placements: BTreeMap<CommentId, Placement>,
}

/// `None` when there's no such item. Comments are placed onto revision
/// `onto`, or the newest when that's not one of the item's.
pub fn conversation<S: ItemStore + ReviewStore + DiffStore + InterdiffStore>(
    store: &S,
    git: &impl DiffGit,
    id: &ItemId,
    onto: Option<u32>,
) -> Result<Option<Conversation>, PlaceError> {
    let Some(item) = store.item(id)? else {
        return Ok(None);
    };
    let newest = item.revision.number;
    let onto = onto.filter(|r| (1..=newest).contains(r)).unwrap_or(newest);
    let mut found = read(store, id, newest)?;
    found.placements = placements(store, git, &item, onto, &found.comments)?;
    Ok(Some(found))
}

/// The conversation of an item the caller has read, its comments unplaced.
pub(crate) fn read<S: ReviewStore>(
    store: &S,
    id: &ItemId,
    newest: u32,
) -> Result<Conversation, StoreError> {
    Ok(Conversation {
        newest,
        comments: store.comments(id)?,
        replies: store.replies(id)?,
        placements: BTreeMap::new(),
    })
}

/// A closed item's diffs were dropped, and nobody reviews it again.
pub(crate) fn placements<S: ItemStore + DiffStore + InterdiffStore>(
    store: &S,
    git: &impl DiffGit,
    item: &Item,
    onto: u32,
    comments: &[Comment],
) -> Result<BTreeMap<CommentId, Placement>, PlaceError> {
    if !item.state.is_open() {
        return Ok(BTreeMap::new());
    }
    place(store, git, &item.id, onto, comments)
}

#[cfg(test)]
#[path = "conversation_tests.rs"]
mod tests;
