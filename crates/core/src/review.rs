//! Reviews: the operator's verdict on a revision, sent to its agent as one
//! `review.submitted` event (design §9.2, F6, F7).

mod error;
mod store;
mod submit;

pub use error::ReviewError;
pub use store::{NewReview, ReviewStore};
pub use submit::{SubmitReview, submit};

use crate::item::ItemId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub id: u64,
    pub item: ItemId,
    /// The revision the review was written on, which may be older than the
    /// item's newest.
    pub revision: u32,
    pub verdict: Verdict,
    /// Empty when the operator wrote none.
    pub summary: String,
    pub submitted_at: String,
}

/// Approving comes with merging, in PR 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Feedback that leaves the item waiting for review.
    Comment,
    /// Sends the item back to its agent.
    ChangesRequested,
}

impl Verdict {
    pub const ALL: [Self; 2] = [Self::Comment, Self::ChangesRequested];

    /// The name the store spells the verdict with.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::ChangesRequested => "changes_requested",
        }
    }
}
