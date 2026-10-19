use serde::{Deserialize, Serialize};

/// A review bot's say on a github item's pull request, from its reaction on
/// the description: 👀 while it reviews, 👍 once it found nothing.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotReview {
    /// Its GitHub login, as in `chatgpt-codex-connector[bot]`.
    pub login: String,
    /// What the operator knows it as, as in `Codex`.
    pub name: String,
    pub state: BotReviewState,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BotReviewState {
    /// A review is running.
    Reviewing,
    /// Its reviews finished without findings.
    Approved,
}
