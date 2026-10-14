//! Submitting a review: the end of F6, and F7's first step.

use super::{NewReview, Reply, Review, ReviewError, ReviewStore, Verdict, reviewable};
use crate::item::{ItemId, ItemState, ItemStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitReview {
    pub id: ItemId,
    pub revision: u32,
    pub verdict: Verdict,
    pub summary: String,
}

/// Request changes sends the item back to its agent; Comment leaves it where
/// it is. Either way the agent gets the review, with every draft and draft
/// reply on the item, as an event. Drafts keep the revision they were written on.
pub fn submit<S: ItemStore + ReviewStore>(
    store: &mut S,
    request: SubmitReview,
) -> Result<Review, ReviewError> {
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
    if revision == 0 || revision > item.revision.number {
        return Err(ReviewError::NoRevision { id, revision });
    }
    let summary = summary.trim().to_owned();
    if summary.is_empty()
        && !store.comments(&id)?.iter().any(|c| c.review.is_none())
        && !store.replies(&id)?.iter().any(Reply::is_draft)
    {
        return Err(ReviewError::Empty);
    }
    let state = match verdict {
        Verdict::ChangesRequested => ItemState::ChangesRequested,
        Verdict::Comment => item.state,
    };
    let review = NewReview {
        item: id,
        revision,
        verdict,
        summary,
    };
    Ok(store.insert_review(review, state)?)
}

#[cfg(test)]
#[path = "submit_tests.rs"]
mod tests;
