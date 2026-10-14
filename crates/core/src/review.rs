//! Reviews: the operator's verdict on a revision and the line comments drafted
//! for it, sent to its agent as one `review.submitted` event, and the replies
//! that thread on each comment (design §9.2, F6, F7).

mod comment;
mod draft;
mod error;
mod reply;
mod store;
mod submit;

pub use comment::{Comment, CommentId, QUOTE_LINES, Side};
pub use draft::{DraftError, QUOTE_LINE_CHARS, WriteDraft, delete_draft, edit_draft, write_draft};
pub use error::ReviewError;
pub use reply::{Author, Reply, ReplyError, ReplyStatus, WriteReply, reply};
pub use store::{NewComment, NewReply, NewReview, ReviewStore};
pub use submit::{SubmitReview, submit};

use crate::item::{Item, ItemId, ItemState};

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
    /// The drafts the review sent, oldest first.
    pub comments: Vec<Comment>,
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

/// Reviews and drafts are taken while the item is with the operator or its
/// agent.
fn reviewable(item: &Item) -> Result<(), ReviewError> {
    match item.state {
        ItemState::NeedsReview | ItemState::ChangesRequested => Ok(()),
        ItemState::Merging => Err(ReviewError::Merging(item.id.clone())),
        ItemState::Merged | ItemState::Abandoned => Err(ReviewError::Closed(item.id.clone())),
    }
}
