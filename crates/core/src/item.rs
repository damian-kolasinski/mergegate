//! Items: units of work submitted for review, one revision per submit.

mod branch;
mod error;
mod git;
mod id;
mod inbox;
mod state;
mod store;
mod submit;

pub use branch::valid_branch;
pub use error::ItemError;
pub use git::{FileStat, ItemGit, valid_commit};
pub use id::{IdError, ItemId};
pub use inbox::{Inbox, inbox};
pub use state::ItemState;
pub use store::{ItemStore, NewItem, NewRevision};
pub use submit::{Submit, Submitted, submit};

use crate::project::Backend;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: ItemId,
    pub branch: String,
    pub title: String,
    pub state: ItemState,
    /// The project's backend and target when the item was opened. The item
    /// keeps them until it closes, even if the project's settings change.
    pub backend: Backend,
    pub target: String,
    /// The newest revision.
    pub revision: Revision,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    pub number: u32,
    pub head: String,
    /// Merge-base of `head` and the item's target when it was submitted.
    pub base: String,
    pub stat: DiffStat,
    pub submitted_at: String,
}

/// Totals over a revision's files. Binary files count as changed files with
/// no lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffStat {
    pub files: u32,
    pub additions: u64,
    pub deletions: u64,
}

impl DiffStat {
    pub fn of(files: &[FileStat]) -> Self {
        Self {
            files: u32::try_from(files.len()).unwrap_or(u32::MAX),
            additions: files.iter().filter_map(|f| f.additions).sum(),
            deletions: files.iter().filter_map(|f| f.deletions).sum(),
        }
    }
}

impl Item {
    /// The revision ref that keeps this revision's commits in staging, even
    /// after its branch is pushed over.
    pub fn pin_ref(&self) -> String {
        format!(
            "refs/mergegate/items/{}/r{}",
            self.id.number(),
            self.revision.number
        )
    }
}
