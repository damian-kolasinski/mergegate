use serde::{Deserialize, Serialize};

use crate::feedback::Source;
use crate::merge::Blocked;
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
    /// The absolute path of the worktree submitting, which `mg resume` names
    /// and a wake command runs in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// The shell command the hub runs when the item has events its agent
    /// hasn't read; empty removes it, none keeps the item's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake: Option<String>,
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
    /// The item on its backend, if it has a page there.
    pub link: Option<Link>,
    pub created_at: String,
}

/// A page outside mergegate, as a pull request on GitHub.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    /// Short, as in `#42`.
    pub label: String,
    /// What the page is, as in `Pull request on GitHub`.
    pub title: String,
    pub url: String,
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
    /// Set while the newest revision's merge is held or conflicts.
    pub blocked: Option<Blocked>,
    pub link: Option<Link>,
    /// Some revision has attachments.
    pub demo: bool,
}
