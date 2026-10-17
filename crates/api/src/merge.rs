//! Approving an item, which squash-merges it, and the held merge it can leave
//! behind.

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
    /// The items stacked on it that its merge moved, bottom first: merged
    /// right after it, or told to rebase.
    #[serde(default)]
    pub stacked: Vec<Item>,
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
    /// Approved, but not tried: it lands right after `parent`, which it's
    /// stacked on.
    Waiting { parent: String },
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
    /// Approved, and lands right after `parent`, which it's stacked on.
    Waiting {
        parent: String,
    },
    /// Approved, but its own diff didn't apply once `parent` merged; it
    /// lands once its agent rebases, on the same approval if the diff is
    /// unchanged.
    Behind {
        parent: String,
    },
}
