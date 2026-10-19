//! Reviews: the operator's verdict on a revision and the line comments drafted
//! for it, sent to its agent as one `review.submitted` event, and the replies
//! that thread on each comment.

mod comment;
mod conversation;
mod draft;
mod draft_reply;
mod error;
mod moves;
mod open;
mod reply;
mod shown;
mod store;
mod submit;
mod waiting;

pub use comment::{Anchor, Comment, CommentId, QUOTE_LINES, Side};
pub use conversation::{Conversation, conversation};
pub(crate) use conversation::{placements, read as read_conversation};
pub(crate) use draft::quote;
pub use draft::{
    DraftError, Lines, MoveDraft, QUOTE_LINE_CHARS, WriteDraft, delete_draft, edit_draft,
    move_draft, write_draft,
};
pub use draft_reply::{DraftReplyError, delete_draft_reply, edit_draft_reply, write_draft_reply};
pub use error::ReviewError;
pub use moves::MovedDraft;
pub(crate) use moves::move_drafts;
pub use open::{Thread, open_threads};
pub use reply::{Author, Reply, ReplyError, ReplyStatus, WriteReply, reply};
pub(crate) use shown::{Shown, shown};
pub use store::{NewComment, NewReply, NewReview, ReviewStore};
pub use submit::{Reviewed, SubmitReview, submit};
pub use waiting::{Waiting, waiting};

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
    /// The operator's draft replies the review sent, oldest first, on
    /// comments earlier reviews sent.
    pub replies: Vec<Reply>,
    pub submitted_at: String,
}

/// What the operator decided about a revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Feedback that leaves the item where it is, unless it sends new
    /// comments on the newest revision, which hand it back to its agent.
    Comment,
    /// Sends the item back to its agent.
    ChangesRequested,
    /// Merges the revision; only [`crate::merge::approve`] sends it.
    Approved,
}

impl Verdict {
    pub const ALL: [Self; 3] = [Self::Comment, Self::ChangesRequested, Self::Approved];

    /// The name the store spells the verdict with.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::ChangesRequested => "changes_requested",
            Self::Approved => "approved",
        }
    }
}

/// Reviews and drafts are taken while the item is with the operator or its
/// agent.
pub(crate) fn reviewable(item: &Item) -> Result<(), ReviewError> {
    match item.state {
        ItemState::NeedsReview | ItemState::ChangesRequested => Ok(()),
        ItemState::Merging => Err(ReviewError::Merging(item.id.clone())),
        ItemState::Merged | ItemState::Abandoned => Err(ReviewError::Closed(item.id.clone())),
    }
}
