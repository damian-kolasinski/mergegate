use crate::feedback::Event;
use crate::github::PullCommentKind;
use crate::item::ItemId;
use crate::review::{Anchor, Comment, CommentId, NewReply, Reply};
use crate::store::StoreError;

/// External comments and the batches that carry them. Times are Unix
/// milliseconds by the hub's clock, which the caller passes.
pub trait ExternalStore {
    /// Records a comment from GitHub, sent from the start and waiting for the
    /// next batch.
    fn insert_external(&mut self, comment: NewExternal, now: u64) -> Result<Comment, StoreError>;

    /// Replaces an external comment's body, and has it wait for the next
    /// batch again.
    fn edit_external(&mut self, id: CommentId, body: &str, now: u64) -> Result<(), StoreError>;

    /// What `id` holds back from its agent; none when nothing waits.
    fn held(&self, id: &ItemId) -> Result<Option<Held>, StoreError>;

    /// Logs every comment `id` holds as one `external.comments` event about
    /// `revision`, with their bodies as they are now, and stops holding them,
    /// all or nothing. None when nothing was held.
    fn release(&mut self, id: &ItemId, revision: u32) -> Result<Option<Event>, StoreError>;

    fn set_agreed(&mut self, id: CommentId, reply: u64) -> Result<(), StoreError>;

    /// Records the operator's override of the agent's rejection, the reply's
    /// `revision` its event's, and appends its `comment.override` event, all
    /// or nothing.
    fn insert_override(&mut self, reply: NewReply) -> Result<(Reply, Event), StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewExternal {
    pub item: ItemId,
    pub revision: u32,
    pub anchor: Option<Anchor>,
    pub body: String,
    pub kind: PullCommentKind,
    pub github_id: u64,
    pub login: String,
    pub bot: bool,
    pub url: String,
    pub path: Option<String>,
}

/// An item's held comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// When the one held longest began waiting.
    pub first: u64,
    /// When the hub last saw one change.
    pub last: u64,
    /// When the item's newest revision was submitted.
    pub pushed: u64,
    /// Who was heard from since then, held or not, as GitHub spells them:
    /// an expected reviewer may run as a plain user account rather than an
    /// app, so this isn't limited to bots.
    pub heard: Vec<String>,
}
