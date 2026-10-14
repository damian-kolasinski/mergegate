use serde::{Deserialize, Serialize};

/// `POST /api/items/{id}/reviews`, answered with [`Review`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitReview {
    /// The revision the review was written on.
    pub revision: u32,
    pub verdict: Verdict,
    #[serde(default)]
    pub summary: String,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Leaves the item waiting for review.
    Comment,
    /// Sends the item back to its agent.
    ChangesRequested,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub id: u64,
    pub revision: u32,
    pub verdict: Verdict,
    pub summary: String,
    pub submitted_at: String,
}
