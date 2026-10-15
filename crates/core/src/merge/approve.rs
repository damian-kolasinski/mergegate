//! Approving a revision, which merges it (F8).

use super::land::{land, through};
use super::{Merge, MergeBackend, MergeError};
use crate::diff::{DiffGit, DiffStore};
use crate::feedback::FeedbackStore;
use crate::highlight::HighlightStore;
use crate::interdiff::InterdiffStore;
use crate::item::{ItemGit, ItemId, ItemState, ItemStore};
use crate::project::ProjectStore;
use crate::review::{NewReview, ReviewStore, Verdict, move_drafts};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approve {
    pub id: ItemId,
    /// The revision the operator approved, which must be the newest: an
    /// older one would merge work they never saw.
    pub revision: u32,
    pub summary: String,
}

/// Sends the item's drafts to its agent as an `approved` review, moving it
/// to Merging, then lands it. The review is recorded first, so the agent
/// learns of the approval even if the merge never finishes. Drafts move to
/// the revision as a submitted review's do.
pub fn approve<
    S: ProjectStore
        + ItemStore
        + ReviewStore
        + DiffStore
        + InterdiffStore
        + HighlightStore
        + FeedbackStore,
>(
    store: &mut S,
    git: &(impl ItemGit + DiffGit),
    backends: &[&dyn MergeBackend],
    request: Approve,
) -> Result<Merge, MergeError> {
    let Approve {
        id,
        revision,
        summary,
    } = request;
    let item = store
        .item(&id)?
        .ok_or_else(|| MergeError::NoItem(id.clone()))?;
    match item.state {
        ItemState::NeedsReview | ItemState::ChangesRequested => {}
        ItemState::Merging => return Err(MergeError::Merging(id)),
        ItemState::Merged | ItemState::Abandoned => return Err(MergeError::Closed(id)),
    }
    if revision == 0 || revision > item.revision.number {
        return Err(MergeError::NoRevision { id, revision });
    }
    if revision != item.revision.number {
        return Err(MergeError::Stale {
            id,
            revision,
            newest: item.revision.number,
        });
    }
    let backend = through(&item, backends)?;
    let moved = move_drafts(store, git, &id, revision)?;
    let review = NewReview {
        item: id.clone(),
        revision,
        verdict: Verdict::Approved,
        summary: summary.trim().to_owned(),
        moved,
    };
    store.insert_review(review, ItemState::Merging)?;
    let item = store.item(&id)?.ok_or(MergeError::NoItem(id))?;
    land(store, git, backend, &item)
}

#[cfg(test)]
#[path = "approve_tests.rs"]
mod tests;
