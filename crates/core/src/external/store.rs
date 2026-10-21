use super::Vote;
use crate::feedback::Event;
use crate::github::PullCommentKind;
use crate::item::ItemId;
use crate::review::{Anchor, Comment, CommentId, NewReply, Reply};
use crate::store::StoreError;

/// External comments and the batches that carry them. Times are Unix
/// milliseconds by the hub's clock, which the caller passes.
pub trait ExternalStore {
    /// Records a comment from GitHub, sent from the start and waiting for the
    /// operator's vote and the next batch, unless it's a summary, which waits
    /// for neither.
    fn insert_external(&mut self, comment: NewExternal, now: u64) -> Result<Comment, StoreError>;

    /// Replaces an external comment's body, which GitHub dates `dated`, and
    /// has it wait for the next batch again, lapsed or not. A vote up was on the old body,
    /// so it's cleared; a comment voted down stays down and doesn't wait.
    /// One that is a `summary` now, with this body, waits for nothing and
    /// loses its vote.
    fn edit_external(
        &mut self,
        id: CommentId,
        body: &str,
        dated: u64,
        summary: bool,
        now: u64,
    ) -> Result<(), StoreError>;

    /// Replaces an external comment's body, which GitHub dates `dated`,
    /// leaving where it stands as it is: one no batch holds stays unheld,
    /// and its vote stays. For an edit that tells the agent nothing new.
    fn amend_external(
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

    /// Logs every comment `id` holds that the operator voted up, as one
    /// `external.comments` event about `revision`, with their bodies as they
    /// are now, and stops holding them, all or nothing. Those awaiting a vote
    /// stay held. None when none may go.
    fn release(&mut self, id: &ItemId, revision: u32) -> Result<Option<Event>, StoreError>;

    /// Stops holding comment `id`, which awaits the operator's vote, as
    /// lapsed: it no longer holds the batch, and a vote up holds it again.
    fn lapse_external(&mut self, id: CommentId) -> Result<(), StoreError>;

    /// Records the operator's vote on external comment `id`: up holds it for the
    /// next batch, down stops holding it. Either way it's no longer lapsed.
    fn set_vote(&mut self, id: CommentId, vote: Vote, now: u64) -> Result<(), StoreError>;

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
    /// A bot's [`summary`](super::summary), which no batch carries.
    pub summary: bool,
    /// When it was posted or last edited, by GitHub's clock.
    pub dated: u64,
}

/// An item's held comments, but those voted down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The operator hasn't voted on some yet.
    pub awaiting: bool,
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
