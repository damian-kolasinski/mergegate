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

/// How urgent an item is; p0 comes first and none sorts after p3.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
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
    /// The agent's review guide, Markdown whose `## Files` list of
    /// `` - `path` — reason `` bullets orders the files to review; blank
    /// removes it, none keeps the last one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide: Option<String>,
    /// From the agent's brief; only the submit that opens the item sets
    /// them, and a later one is warned to use `mg prioritize`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `POST /api/items/{id}/priority`, answered with the [`Item`]. It sets both:
/// none clears the priority, and a missing note clears the old one.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prioritize {
    #[serde(default)]
    pub priority: Option<Priority>,
    #[serde(default)]
    pub note: Option<String>,
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
    /// What the agent should fix in the revision's guide, as in
    /// ``"`src/old.rs` isn't changed in this revision"``.
    #[serde(default)]
    pub warnings: Vec<String>,
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
    pub priority: Option<Priority>,
    /// Why it has its priority.
    pub note: Option<String>,
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
    /// The files in the review tier, which trusted ones leave out.
    pub review_files: u32,
    pub submitted_at: String,
}

/// `GET /api/inbox`: open items, each group by priority, then oldest first.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Inbox {
    pub needs_you: Vec<InboxItem>,
    pub with_agents: Vec<InboxItem>,
    pub merging: Vec<InboxItem>,
    /// Projects with changes to their rules waiting for the operator.
    pub rules: Vec<crate::rules::PendingRules>,
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
    /// As in [`Revision`].
    pub review_files: u32,
    pub created_at: String,
    /// When the newest revision was submitted.
    pub updated_at: String,
    /// Set while the newest revision's merge is held or conflicts.
    pub blocked: Option<Blocked>,
    pub link: Option<Link>,
    /// Some revision has attachments.
    pub demo: bool,
    /// Set when a revision was submitted since the last reviewed one.
    pub changed_since: Option<ChangedSince>,
    pub priority: Option<Priority>,
    pub note: Option<String>,
    /// The open items its head conflicts with, of its project and target,
    /// oldest first: landing one of them hands this one back.
    pub conflicts_with: Vec<String>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedSince {
    /// The last reviewed revision.
    pub revision: u32,
    /// The files changed since it.
    pub files: u32,
}
