//! Submitting a review, which sends its comments to the agent.

use super::{
    Comment, NewReview, Reply, Review, ReviewError, ReviewStore, Verdict, move_drafts, reviewable,
};
use crate::diff::{DiffGit, DiffStore};
use crate::interdiff::InterdiffStore;
use crate::item::{ItemId, ItemState, ItemStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitReview {
    pub id: ItemId,
    pub revision: u32,
    pub verdict: Verdict,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reviewed {
    pub review: Review,
    /// Where the review left the item.
    pub state: ItemState,
}

/// Request changes sends the item back to its agent. So does a comment that
/// sends new comments on the newest revision, which the agent acts on
/// whatever the verdict; any other comment leaves the item where it is.
/// Either way the agent gets the review, with every draft and draft reply on
/// the item, as an event. Drafts from earlier revisions go on the
/// review's revision when their lines didn't change; one from a later
/// revision keeps its own.
pub fn submit<S: ItemStore + ReviewStore + DiffStore + InterdiffStore>(
    store: &mut S,
    git: &impl DiffGit,
    request: SubmitReview,
) -> Result<Reviewed, ReviewError> {
    let SubmitReview {
        id,
        revision,
        verdict,
        summary,
    } = request;
    let item = store
        .item(&id)?
        .ok_or_else(|| ReviewError::NoItem(id.clone()))?;
    reviewable(&item)?;
    // An approval that didn't merge would leave the agent waiting on a merge
    // that never comes.
    if verdict == Verdict::Approved {
        return Err(ReviewError::Approval);
    }
    if revision == 0 || revision > item.revision.number {
        return Err(ReviewError::NoRevision { id, revision });
    }
    // The agent already sent a newer revision, which sending the item back
    // would hand back unread. A comment on the older one still goes out.
    if verdict == Verdict::ChangesRequested && revision < item.revision.number {
        return Err(ReviewError::Stale {
            id,
            revision,
            newest: item.revision.number,
        });
    }
    let summary = summary.trim().to_owned();
    let drafts: Vec<Comment> = store
        .comments(&id)?
        .into_iter()
        .filter(Comment::is_draft)
        .collect();
    if summary.is_empty() && drafts.is_empty() && !store.replies(&id)?.iter().any(Reply::is_draft) {
        return Err(ReviewError::Empty);
    }
    // Every draft lands on the newest revision when the review is on it; on
    // an older one, only those written on the newest keep it. Comments on an
    // older revision alone leave the newest waiting for review.
    let newest = item.revision.number;
    let on_newest = drafts
        .iter()
        .any(|draft| revision == newest || draft.revision == newest);
    let state = match verdict {
        Verdict::ChangesRequested => ItemState::ChangesRequested,
        Verdict::Comment if on_newest => ItemState::ChangesRequested,
        Verdict::Comment | Verdict::Approved => item.state,
    };
    let moved = move_drafts(store, git, &id, revision)?;
    let review = NewReview {
        item: id,
        revision,
        verdict,
        summary,
        moved,
        carried_from: None,
    };
    let review = store.insert_review(review, state)?;
    Ok(Reviewed { review, state })
}

#[cfg(test)]
#[path = "submit_tests.rs"]
mod tests;
