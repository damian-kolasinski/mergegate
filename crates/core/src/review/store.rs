use super::{Review, Verdict};
use crate::item::{ItemId, ItemState};
use crate::store::StoreError;

/// Reviews and the events that carry them. The store stamps submit times.
pub trait ReviewStore {
    /// Records the review, moves the item to `state` and appends the review's
    /// `review.submitted` event, all or nothing: an agent never misses a
    /// review the operator saw go through.
    fn insert_review(&mut self, review: NewReview, state: ItemState) -> Result<Review, StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewReview {
    pub item: ItemId,
    pub revision: u32,
    pub verdict: Verdict,
    pub summary: String,
}
