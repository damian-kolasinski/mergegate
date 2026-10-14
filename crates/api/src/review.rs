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
    /// The drafts the review sent, oldest first.
    pub comments: Vec<Comment>,
    pub submitted_at: String,
}

/// A comment on lines `start_line..=line` of one side of a file.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// Hub-wide, as in `c_17`.
    pub id: String,
    /// The revision it was written on.
    pub revision: u32,
    pub path: String,
    pub side: Side,
    pub start_line: u32,
    pub line: u32,
    pub body: String,
    /// The last few of the commented lines.
    pub quote: String,
}

/// The file before the change, or after it.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Old,
    New,
}

/// `POST /api/items/{id}/drafts`, answered with the draft's [`Comment`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteDraft {
    pub revision: u32,
    pub path: String,
    pub side: Side,
    pub start_line: u32,
    pub line: u32,
    pub body: String,
}

/// `PUT /api/items/{id}/drafts/{comment}`, answered with the [`Comment`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditDraft {
    pub body: String,
}
