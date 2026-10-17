//! Replies: each sent comment's thread. The agent answers with a status, and
//! its latest one counts.

use super::{CommentId, NewReply, ReviewError, ReviewStore, reviewable};
use crate::item::{ItemId, ItemStore};
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub id: u64,
    pub comment: CommentId,
    pub item: ItemId,
    pub author: Author,
    /// The agent's; the operator's replies have none.
    pub status: Option<ReplyStatus>,
    pub body: String,
    /// The item's newest revision when the reply was written.
    pub revision: u32,
    /// The review that sent an operator's reply; none while it's a draft,
    /// and none on the agent's or an override, which go out as they're
    /// written.
    pub review: Option<u64>,
    /// The operator's override of the agent's rejection, which reopens the
    /// thread; its body may be empty.
    pub overrides: bool,
    pub created_at: String,
}

impl Reply {
    /// The operator's, waiting for the next review to send it.
    pub fn is_draft(&self) -> bool {
        self.author == Author::Operator && self.review.is_none() && !self.overrides
    }

    /// The revision an `addressed` reply was addressed in: the first one
    /// submitted after it, once there is one.
    pub fn addressed_in(&self, newest: u32) -> Option<u32> {
        (self.status == Some(ReplyStatus::Addressed) && newest > self.revision)
            .then_some(self.revision + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Author {
    Agent,
    Operator,
}

impl Author {
    pub const ALL: [Self; 2] = [Self::Agent, Self::Operator];

    /// The name the store spells the author with.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Operator => "operator",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyStatus {
    /// The next revision has the change.
    Addressed,
    /// The agent won't change it, and says why.
    Rejected,
    /// The agent needs the operator to answer first.
    Question,
}

impl ReplyStatus {
    pub const ALL: [Self; 3] = [Self::Addressed, Self::Rejected, Self::Question];

    /// The name the store and `mg reply --status` spell the status with.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Addressed => "addressed",
            Self::Rejected => "rejected",
            Self::Question => "question",
        }
    }
}

/// The agent's answer to a sent comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteReply {
    pub comment: CommentId,
    pub status: ReplyStatus,
    pub body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ReplyError {
    /// The comment's item takes no more feedback.
    #[error(transparent)]
    Review(#[from] ReviewError),
    /// Drafts are the operator's until a review sends them, so the agent
    /// can't name one either.
    #[error("no comment {0}")]
    NoComment(CommentId),
    #[error("a {} reply needs a body saying why", .0.as_str())]
    NeedsBody(ReplyStatus),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Appends the agent's reply to a sent comment's thread. A rejection or a
/// question says why; `addressed` may leave it to the next revision.
pub fn reply<S: ItemStore + ReviewStore>(
    store: &mut S,
    request: WriteReply,
) -> Result<Reply, ReplyError> {
    let WriteReply {
        comment,
        status,
        body,
    } = request;
    let found = store
        .comment(comment)?
        .filter(|c| !c.is_draft())
        .ok_or(ReplyError::NoComment(comment))?;
    let item = store
        .item(&found.item)?
        .ok_or_else(|| ReviewError::NoItem(found.item.clone()))?;
    reviewable(&item)?;
    let body = body.trim().to_owned();
    if body.is_empty() && status != ReplyStatus::Addressed {
        return Err(ReplyError::NeedsBody(status));
    }
    let reply = NewReply {
        comment,
        item: item.id,
        author: Author::Agent,
        status: Some(status),
        body,
        revision: item.revision.number,
    };
    Ok(store.insert_reply(reply)?)
}

#[cfg(test)]
#[path = "reply_tests.rs"]
mod tests;
