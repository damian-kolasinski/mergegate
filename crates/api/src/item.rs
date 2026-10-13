use serde::{Deserialize, Serialize};

use crate::project::Backend;

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

    pub fn is_open(self) -> bool {
        !matches!(self, Self::Merged | Self::Abandoned)
    }
}

/// `POST /api/items`, answered with [`Submitted`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submit {
    pub project: String,
    pub branch: String,
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
    /// As in `app#42`.
    pub id: String,
    pub title: String,
    pub branch: String,
    pub state: ItemState,
    /// Kept from when the item was opened, whatever the project says now.
    pub backend: Backend,
    pub target: String,
    /// The newest one.
    pub revision: Revision,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    pub number: u32,
    pub head: String,
    /// Merge-base with the target at submit time.
    pub base: String,
    pub files: u32,
    pub additions: u64,
    pub deletions: u64,
    pub submitted_at: String,
}

/// `GET /api/inbox`: open items, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Inbox {
    pub needs_you: Vec<InboxItem>,
    pub with_agents: Vec<InboxItem>,
    pub merging: Vec<InboxItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: String,
    pub title: String,
    pub branch: String,
    /// The newest revision's number.
    pub revision: u32,
    pub files: u32,
    pub additions: u64,
    pub deletions: u64,
    pub created_at: String,
    /// When the newest revision was submitted.
    pub updated_at: String,
}
