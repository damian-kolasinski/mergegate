//! A held merge, or one a restart cut short, waits in Merging for the
//! operator to retry or cancel it; nothing retries on its own.

use super::land::{land, through};
use super::{Merge, MergeBackend, MergeError};
use crate::diff::DiffStore;
use crate::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use crate::item::{Item, ItemGit, ItemId, ItemState, ItemStore};
use crate::project::ProjectStore;

/// Lands the item again, as approve would have.
pub fn retry<S: ProjectStore + ItemStore + DiffStore + FeedbackStore>(
    store: &mut S,
    git: &impl ItemGit,
    backends: &[&dyn MergeBackend],
    id: &ItemId,
) -> Result<Merge, MergeError> {
    let item = merging(store, id)?;
    let backend = through(&item, backends)?;
    land(store, git, backend, &item)
}

/// Puts the item back in Needs review, and tells its agent the approval is
/// off.
pub fn cancel<S: ItemStore + FeedbackStore>(
    store: &mut S,
    id: &ItemId,
) -> Result<Item, MergeError> {
    let item = merging(store, id)?;
    let notice = Notice {
        revision: item.revision.number,
        source: Source::Operator,
        kind: NoticeKind::MergeCancelled,
    };
    store.notify(id, ItemState::NeedsReview, notice)?;
    store
        .item(id)?
        .ok_or_else(|| MergeError::NoItem(id.clone()))
}

fn merging(store: &impl ItemStore, id: &ItemId) -> Result<Item, MergeError> {
    let item = store
        .item(id)?
        .ok_or_else(|| MergeError::NoItem(id.clone()))?;
    match item.state {
        ItemState::Merging => Ok(item),
        ItemState::Merged | ItemState::Abandoned => Err(MergeError::Closed(id.clone())),
        ItemState::NeedsReview | ItemState::ChangesRequested => {
            Err(MergeError::NotMerging(id.clone()))
        }
    }
}

#[cfg(test)]
#[path = "hold_tests.rs"]
mod tests;
