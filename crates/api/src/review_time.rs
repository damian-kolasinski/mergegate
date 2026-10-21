//! How long the operator spends reviewing an item, as the UI measures it,
//! and the stats closing events carry for agents' telemetry.

use serde::{Deserialize, Serialize};

use crate::review::Verdict;

/// `POST /api/items/{id}/review-time`: time the operator was active on the
/// item's review screen since the UI's last report. Refused once the item
/// closed, so its stats stay as its closing event first reported them.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewTime {
    /// The revision on screen.
    pub revision: u32,
    pub ms: u64,
}

/// On `item.merged` and `item.abandoned`: how the item's review went.
/// Timestamps are UTC, RFC 3339.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewStats {
    /// When the first revision was submitted.
    pub opened_at: String,
    /// None for an item closed before the hub recorded when.
    pub closed_at: Option<String>,
    /// The operator's active time on the review screen, all of it.
    pub review_ms: u64,
    /// The operator's review rounds, oldest first: each review they sent
    /// ends one, the approval included. The last has no verdict when the
    /// item closed with one under way, abandoned or merged on GitHub.
    pub turns: Vec<TurnStats>,
    /// Oldest first. A revision the agent submitted without a review in
    /// between, such as a rebase, has its own entry but no turn.
    pub revisions: Vec<RevisionStats>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnStats {
    /// From 1.
    pub turn: u32,
    /// The revision the turn's review was sent on; for a turn under way, the
    /// newest it had time on.
    pub revision: u32,
    /// None for a turn under way.
    pub verdict: Option<Verdict>,
    /// When its review was sent; None for a turn under way.
    pub sent_at: Option<String>,
    /// Active time since the previous review was sent.
    pub review_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionStats {
    pub revision: u32,
    pub submitted_at: String,
    /// Active time with this revision on screen.
    pub review_ms: u64,
}
