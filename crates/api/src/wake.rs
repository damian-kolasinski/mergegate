//! `GET /api/wakes`: the wake commands a device's `mg wakes` runner runs for
//! the items its agents submitted from it. Its query: `wait`, the seconds to
//! wait for one, and `running`, the comma-separated IDs whose commands it is
//! still running.

use serde::{Deserialize, Serialize};

/// The wakes the hub handed this runner, each recorded as woken: the hub
/// doesn't hand one over again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Due {
    pub wakes: Vec<Wake>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wake {
    /// The item's ID on the hub, which the runner's polls list while its
    /// command runs.
    pub id: String,
    /// The item's name, as in `app#3`.
    pub item: String,
    /// The command to run with `sh -c`, `{item}` already replaced.
    pub command: String,
    /// The absolute path, on the runner's machine, to run it in.
    pub worktree: String,
    /// The item's newest event from the operator.
    pub cursor: u64,
}
