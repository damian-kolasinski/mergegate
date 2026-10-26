//! What `mg resume` turns into a prompt for a fresh agent session.

use serde::{Deserialize, Serialize};

use crate::feedback::Event;
use crate::item::Item;
use crate::review::{Comment, Reply, Verdict};

/// `GET /api/items/{id}/resume`. Reading it acknowledges no events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resume {
    pub item: Item,
    /// Where the newest submit came from.
    pub worktree: Option<String>,
    /// The wake command the newest submit left.
    pub wake: Option<String>,
    /// The newest review the operator sent.
    pub review: Option<LastReview>,
    /// The sent comments the agent hasn't settled, oldest first.
    pub open: Vec<Thread>,
    /// The notice that asks for another revision while it still stands, as
    /// a conflict with the target; none from an older hub.
    #[serde(default)]
    pub pending: Option<Event>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastReview {
    pub revision: u32,
    pub verdict: Verdict,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    pub comment: Comment,
    /// Sent ones, oldest first.
    pub replies: Vec<Reply>,
}
