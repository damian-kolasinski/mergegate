use super::{Author, Comment, CommentId, Reply, ReplyStatus, Review, Side, Verdict};
use crate::item::{ItemId, ItemState};
use crate::store::StoreError;

/// Reviews, their comments and the events that carry them. The store stamps
/// submit and create times.
pub trait ReviewStore {
    /// Records the review with every draft and draft reply on its item, moves the item to
    /// `state` and appends the review's `review.submitted` event, all or
    /// nothing: an agent never misses a review the operator saw go through.
    fn insert_review(&mut self, review: NewReview, state: ItemState) -> Result<Review, StoreError>;

    fn insert_draft(&mut self, comment: NewComment) -> Result<Comment, StoreError>;

    /// Every comment on `id`, drafts and sent ones, oldest first.
    fn comments(&self, id: &ItemId) -> Result<Vec<Comment>, StoreError>;

    /// Changes nothing once the comment was sent.
    fn set_draft_body(&mut self, id: CommentId, body: &str) -> Result<(), StoreError>;

    /// Deletes nothing once the comment was sent.
    fn delete_draft(&mut self, id: CommentId) -> Result<(), StoreError>;

    /// The comment, on whichever item, draft or sent.
    fn comment(&self, id: CommentId) -> Result<Option<Comment>, StoreError>;

    fn insert_reply(&mut self, reply: NewReply) -> Result<Reply, StoreError>;

    /// The reply, on whichever item, draft or sent.
    fn reply(&self, id: u64) -> Result<Option<Reply>, StoreError>;

    /// Changes nothing once the reply was sent.
    fn set_draft_reply_body(&mut self, id: u64, body: &str) -> Result<(), StoreError>;

    /// Deletes nothing once the reply was sent.
    fn delete_draft_reply(&mut self, id: u64) -> Result<(), StoreError>;

    /// Every reply on `id`'s comments, oldest first.
    fn replies(&self, id: &ItemId) -> Result<Vec<Reply>, StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewReview {
    pub item: ItemId,
    pub revision: u32,
    pub verdict: Verdict,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewComment {
    pub item: ItemId,
    pub revision: u32,
    pub path: String,
    pub side: Side,
    pub start_line: u32,
    pub line: u32,
    pub body: String,
    pub quote: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewReply {
    pub comment: CommentId,
    pub item: ItemId,
    pub author: Author,
    pub status: Option<ReplyStatus>,
    pub body: String,
    pub revision: u32,
}
