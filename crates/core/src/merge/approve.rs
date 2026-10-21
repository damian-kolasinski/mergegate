//! Approving a revision, which merges it.

use super::land::{land, through};
use super::stacked::tidy_parent;
use super::{Landing, Merge, MergeBackend, MergeError};
use crate::diff::{DiffGit, DiffStore};
use crate::feedback::FeedbackStore;
use crate::highlight::HighlightStore;
use crate::interdiff::InterdiffStore;
use crate::item::{ItemGit, ItemId, ItemState, ItemStore};
use crate::project::ProjectStore;
use crate::review::{NewReview, ReviewStore, Verdict, move_drafts};
use crate::store::Shared;

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
/// the revision as a submitted review's do. An item stacked on one that
/// hasn't landed waits in Merging and lands right after it.
pub fn approve<
    S: ProjectStore
        + ItemStore
        + ReviewStore
        + DiffStore
        + InterdiffStore
        + HighlightStore
        + FeedbackStore,
>(
    store: &impl Shared<S>,
    git: &(impl ItemGit + DiffGit),
    backends: &[&dyn MergeBackend],
    request: Approve,
) -> Result<Merge, MergeError> {
    let Approve {
        id,
        revision,
        summary,
    } = request;
    // One step, so nothing approves it twice: the next finds it Merging.
    let (item, backend) = store.with(|store| -> Result<_, MergeError> {
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
        let parent = match &item.parent {
            Some(parent) => store.item(parent)?,
            None => None,
        };
        if let Some(parent) = parent.filter(|parent| parent.state == ItemState::Abandoned) {
            return Err(MergeError::ParentAbandoned {
                id,
                parent: parent.name(),
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
            carried_from: None,
        };
        store.insert_review(review, ItemState::Merging)?;
        let item = store.item(&id)?.ok_or(MergeError::NoItem(id))?;
        Ok((item, backend))
    })?;
    let merge = land(store, git, backend, &item)?;
    // A child its parent's merge couldn't move off that parent's branch.
    if matches!(merge.landing, Landing::Merged { .. }) {
        tidy_parent(store, backend, &item)?;
    }
    Ok(merge)
}

#[cfg(test)]
#[path = "approve_tests.rs"]
mod tests;
