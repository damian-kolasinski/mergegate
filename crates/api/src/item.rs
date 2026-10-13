use serde::{Deserialize, Serialize};

use crate::project::Backend;

/// Where an item is in its lifecycle (design §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    NeedsReview,
    ChangesRequested,
    Merging,
    Merged,
    Abandoned,
}

impl ItemState {
    pub const ALL: [Self; 5] = [
        Self::NeedsReview,
        Self::ChangesRequested,
        Self::Merging,
        Self::Merged,
        Self::Abandoned,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeedsReview => "needs_review",
            Self::ChangesRequested => "changes_requested",
            Self::Merging => "merging",
            Self::Merged => "merged",
            Self::Abandoned => "abandoned",
        }
    }

    /// Whether the item can still change: not merged or abandoned.
    pub fn is_open(self) -> bool {
        !matches!(self, Self::Merged | Self::Abandoned)
    }
}

/// `POST /api/items`: registers a pushed branch as a new item, or as a new
/// revision of the branch's open item. Answers with [`Submitted`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submit {
    pub project: String,
    pub branch: String,
    /// The commit the worker pushed the branch to.
    pub head: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submitted {
    pub item: Item,
    /// False when the branch was already submitted at this head.
    pub new_revision: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Project-scoped, as in `app#42`.
    pub id: String,
    pub title: String,
    pub branch: String,
    pub state: ItemState,
    /// The project's backend and target when the item was opened; the item
    /// keeps them until it closes.
    pub backend: Backend,
    pub target: String,
    /// The newest revision.
    pub revision: Revision,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    /// 1 for the first submit, then counting up.
    pub number: u32,
    pub head: String,
    /// Merge-base of the head and the target when it was submitted.
    pub base: String,
    pub files: u32,
    pub additions: u64,
    pub deletions: u64,
    pub submitted_at: String,
}

/// `GET /api/inbox`: open items by who has to act next, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Inbox {
    /// Waiting for the operator's review.
    pub needs_you: Vec<InboxItem>,
    /// Sent back to their agents.
    pub with_agents: Vec<InboxItem>,
    pub merging: Vec<InboxItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: String,
    pub title: String,
    pub branch: String,
    /// Number of the newest revision.
    pub revision: u32,
    pub files: u32,
    pub additions: u64,
    pub deletions: u64,
    pub created_at: String,
    /// When the newest revision was submitted.
    pub updated_at: String,
}
