//! `POST /api/attention?since=&wait=`: what a Claude Code session's Stop
//! hook watches between turns, in one long poll. Read-only: unlike feedback,
//! it never hands events to an agent or releases held external comments.
//! A POST because the worktrees it names don't fit a query string; `since`
//! and `wait` stay in the query, as on the other long polls.

use serde::{Deserialize, Serialize};

use crate::item::ItemState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watch {
    /// The clone the session ends its turns in, whose items it watches while
    /// they're open or closed today.
    pub worktree: Option<String>,
    /// Items watched wherever the session is.
    pub items: Vec<Submitted>,
}

/// An item and the worktree a session sent it from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submitted {
    pub id: String,
    pub worktree: String,
}

/// Answered at once without `since` or `wait`, otherwise once a watched item
/// has events after `since` or the wait is up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watching {
    /// The items whose newest submit came from the named worktree, on the
    /// calling device, without a wake command, which would wake its agent
    /// instead.
    pub items: Vec<Watched>,
    /// The `since` of the next wait.
    pub cursor: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watched {
    pub id: String,
    /// As in [`crate::item::Item`].
    pub name: String,
    pub state: ItemState,
    /// The newest event its agent hasn't read that is worth waking it for.
    pub cursor: Option<u64>,
}
