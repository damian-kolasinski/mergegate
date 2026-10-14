//! Approving an item, which squash-merges it, and the held merge it can leave
//! behind (design §5, F8).

use serde::{Deserialize, Serialize};

use crate::item::Item;

/// `POST /api/items/{id}/approve`, answered with [`Merge`]. Sends the drafts
/// as an `approved` review, then lands the revision on the target as one
/// commit.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approve {
    /// Must be the item's newest.
    pub revision: u32,
    #[serde(default)]
    pub summary: String,
}

/// What an approve or a retry (`POST /api/items/{id}/merge`) did.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Merge {
    pub item: Item,
    pub outcome: Outcome,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    /// Closed as merged at `commit` on the target.
    Merged { commit: String },
    /// Still in Merging, until the operator retries or cancels
    /// (`DELETE /api/items/{id}/merge`).
    Held { reason: String },
    /// Back with its agent to rebase.
    Conflict {
        target_commit: String,
        paths: Vec<String>,
    },
}

/// Why an open item's merge didn't land, while that still stands.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Blocked {
    Held {
        reason: String,
    },
    Conflict {
        target_commit: String,
        paths: Vec<String>,
    },
}
