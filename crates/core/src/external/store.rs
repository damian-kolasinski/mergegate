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

    /// Replaces an external comment's body, which GitHub dates `dated`, and
    /// has it wait for the next batch again.
    fn edit_external(
        &mut self,
        id: CommentId,
        body: &str,
        dated: u64,
        now: u64,
    ) -> Result<(), StoreError>;

    /// Withdraws held comment `id`, which GitHub no longer has. One no batch
    /// carried and no one answered is deleted, with the operator's draft
    /// replies on it, so it never reaches the agent; any other stops waiting
    /// with the body the agent last got, as the agent may have acted on it.
    fn withdraw_external(&mut self, id: CommentId) -> Result<(), StoreError>;

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
    /// When it was posted or last edited, by GitHub's clock.
    pub dated: u64,
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
    /// Who posted or edited a comment since then by GitHub's clock, held or
    /// not, as GitHub spells them: an expected reviewer may run as a plain
    /// user account rather than an app, so this isn't limited to bots. When
    /// the hub saw it doesn't count, as a review of the revision before may
    /// reach the hub only after the push.
    pub heard: Vec<String>,
}
