//! What `mg wait` and `mg feedback` read: an item's events (design §6.3).

use serde::{Deserialize, Serialize};

use crate::item::ItemState;
use crate::review::{Comment, Reply, Verdict};

/// `GET /api/items/{id}/feedback`: the events after `since`, or after the
/// item's delivered cursor. With `wait` (seconds), the hub holds the request
/// until there is an event that `for` wakes on, the item closes, or the time
/// is up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    pub item: String,
    /// The item's name, as in [`crate::item::Item`].
    pub name: String,
    pub state: ItemState,
    /// Oldest first.
    pub events: Vec<Event>,
    /// Where the next read starts; acknowledge it once the events are handled.
    pub cursor: u64,
}

/// `GET /api/events`: every item's events after `since`, which
/// `mg events --follow` streams. Without `since`, none and the newest cursor,
/// to start from now. With `wait` (seconds), the hub holds the request until
/// there is one or the time is up. It acknowledges nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Log {
    /// Oldest first.
    pub events: Vec<Event>,
    /// Where the next read starts.
    pub cursor: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub cursor: u64,
    pub item: String,
    /// The item's name, as in [`crate::item::Item`].
    pub name: String,
    /// The revision the event is about.
    pub revision: u32,
    pub source: Source,
    #[serde(flatten)]
    pub body: EventBody,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Operator,
    /// The item's own agent, through `mg`.
    Agent,
    /// Reviewers on the item's pull request: bots and teammates.
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventBody {
    #[serde(rename = "review.submitted")]
    ReviewSubmitted {
        verdict: Verdict,
        summary: String,
        comments: Vec<Comment>,
        /// The operator's answers on comments earlier reviews sent.
        replies: Vec<Reply>,
    },
    /// The item closed without merging; `source` says who closed it.
    #[serde(rename = "item.abandoned")]
    ItemAbandoned {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// The approved revision didn't land; the item waits in Merging for the
    /// operator to retry or cancel. `reason` is git's.
    #[serde(rename = "merge.held")]
    MergeHeld { reason: String },
    /// The operator gave up on the held merge; the item needs review again.
    #[serde(rename = "merge.cancelled")]
    MergeCancelled {},
    /// The approved revision conflicts with the target at `target_commit`,
    /// so the item is back with its agent, who runs `commands` in its
    /// worktree to rebase, then submits again.
    #[serde(rename = "merge.conflict")]
    MergeConflict {
        target_commit: String,
        paths: Vec<String>,
        commands: Vec<String>,
    },
    /// Comments reviewers left on the item's pull request, held until they
    /// went quiet. Inline ones are on lines like the operator's; the rest
    /// are on the item as a whole. Answer each with `mg reply`.
    #[serde(rename = "external.comments")]
    ExternalComments {
        /// Oldest first.
        comments: Vec<Comment>,
        /// Those an earlier `external.comments` carried with another body.
        edited: Vec<String>,
    },
    /// The operator wants the change the agent rejected on `comment`;
    /// `reply.body` says why, when they wrote it.
    #[serde(rename = "comment.override")]
    CommentOverride { comment: Box<Comment>, reply: Reply },
    /// The approved revision landed on the target as `commit`. A squash
    /// leaves `branch` unmerged to git, so deleting it takes `git branch -D`.
    /// `commands` is the default cleanup, for an agent that agreed no other
    /// with the operator: it leaves the worktree on the updated target. A
    /// hub older than it sends none.
    #[serde(rename = "item.merged")]
    ItemMerged {
        commit: String,
        branch: String,
        #[serde(default)]
        commands: Vec<String>,
    },
}

impl EventBody {
    /// The `type` it is serialized under.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ReviewSubmitted { .. } => "review.submitted",
            Self::ItemAbandoned { .. } => "item.abandoned",
            Self::MergeHeld { .. } => "merge.held",
            Self::MergeCancelled {} => "merge.cancelled",
            Self::MergeConflict { .. } => "merge.conflict",
            Self::ExternalComments { .. } => "external.comments",
            Self::CommentOverride { .. } => "comment.override",
            Self::ItemMerged { .. } => "item.merged",
        }
    }
}

/// Which events end a wait; `for` on the feedback route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wake {
    #[default]
    Any,
    Operator,
}

/// `POST /api/items/{id}/feedback/ack`: the events up to `cursor` were
/// handled. Answered with [`Acknowledged`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acknowledge {
    pub cursor: u64,
    /// The `since` of the read that returned the events. A read that started
    /// past an event not yet delivered leaves the delivered cursor where it
    /// is, as that event never reached the agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acknowledged {
    /// The item's delivered cursor now, which an older `cursor` doesn't move.
    pub delivered: u64,
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
