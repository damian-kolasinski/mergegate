//! Items: units of work submitted for review, one revision per submit.

mod abandon;
mod backend;
mod branch;
mod close;
mod diff_files;
mod drop_branch;
mod error;
mod follow_pull;
mod git;
mod id;
mod inbox;
mod item_diff;
mod mode;
mod name;
mod priority;
mod publish;
mod stack;
mod state;
mod store;
mod submit;
mod weekly;

pub use abandon::{Abandon, abandon};
pub use backend::ItemBackend;
pub use branch::valid_branch;
pub(crate) use close::close;
pub(crate) use diff_files::diff_files;
pub use drop_branch::{drop_branch, drop_closed_branches};
pub use error::ItemError;
pub use follow_pull::{CLOSED_ON_GITHUB, follow_pull};
pub use git::{ChangedFile, FileStat, ItemGit, Pushed, valid_commit};
pub use id::{IdError, ItemId};
pub use inbox::{Inbox, inbox};
pub use item_diff::{Compare, DiffedFile, ItemDiff, item_diff};
pub use mode::FileMode;
pub use name::named;
pub use priority::{Prioritize, Priority, prioritize};
pub use stack::{On, Stack, StackError, behind, stack};
pub use state::ItemState;
pub use store::{ItemStore, NewItem, NewRevision};
pub use submit::{Submit, Submitted, submit};
pub use weekly::{RepositoryClosures, WeeklyClosures};

use crate::triage::Tier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: ItemId,
    pub branch: String,
    pub title: String,
    pub state: ItemState,
    /// The project's backend and target when the item was opened. The item
    /// keeps them until it closes, even if the project's settings change.
    pub backend: ItemBackend,
    pub target: String,
    /// The newest revision.
    pub revision: Revision,
    pub created_at: String,
    /// When it merged or was abandoned; none while it's open.
    pub closed_at: Option<String>,
    /// None sorts after p3.
    pub priority: Option<Priority>,
    /// Why it has its priority, for the operator.
    pub note: Option<String>,
    /// The item it's stacked on, set by `mg submit --on`. It stays
    /// once the parent closes, until the item's next revision.
    pub parent: Option<ItemId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    pub number: u32,
    pub head: String,
    /// What the revision's diff is against: the merge-base of `head` and
    /// the item's target when it was submitted, or for a stacked item the
    /// head of the newest parent revision `head` contains.
    pub base: String,
    pub stat: DiffStat,
    pub submitted_at: String,
}

/// Totals over a revision's files. Files whose change isn't text count as
/// changed files with no lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffStat {
    pub files: u32,
    pub additions: u64,
    pub deletions: u64,
    /// The files in the `review` tier, which trusted ones leave out.
    pub review_files: u32,
}

impl DiffStat {
    pub fn of(files: &[FileStat]) -> Self {
        let review = files.iter().filter(|f| f.triage.tier == Tier::Review);
        Self {
            files: u32::try_from(files.len()).unwrap_or(u32::MAX),
            additions: files.iter().filter_map(|f| f.additions).sum(),
            deletions: files.iter().filter_map(|f| f.deletions).sum(),
            review_files: u32::try_from(review.count()).unwrap_or(u32::MAX),
        }
    }
}

impl Item {
    /// When it last moved on: as it closed, else as its newest revision
    /// came in.
    pub fn updated_at(&self) -> &str {
        self.closed_at
            .as_deref()
            .unwrap_or(&self.revision.submitted_at)
    }

    /// The revision ref that keeps this revision's commits in staging, even
    /// after its branch is pushed over.
    pub fn pin_ref(&self) -> String {
        format!("{}r{}", self.pins(), self.revision.number)
    }

    /// The prefix of every revision ref the item has.
    pub fn pins(&self) -> String {
        format!("refs/mergegate/items/{}/", self.id.number())
    }
}
