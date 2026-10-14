use serde::{Deserialize, Serialize};

use crate::feedback::Source;
use crate::project::Backend;

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    NeedsReview,
    ChangesRequested,
    Merging,
    Merged,
    Abandoned,
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

/// `POST /api/items/{id}/abandon`, answered with the closed [`Item`]. Its
/// agent reads who closed it and why in an `item.abandoned` event.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abandon {
    pub source: Source,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submitted {
    pub item: Item,
    /// False when the branch was already submitted at this head.
    pub new_revision: bool,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Inbox {
    pub needs_you: Vec<InboxItem>,
    pub with_agents: Vec<InboxItem>,
    pub merging: Vec<InboxItem>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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
