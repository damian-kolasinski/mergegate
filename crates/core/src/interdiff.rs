//! Interdiffs: what a revision changes since an earlier one of its item, with
//! the target's own changes left out when the base moved (F7, design §9.2).

mod changed;
mod git;
mod record;
mod store;

pub use changed::{ChangedSince, changed_since_review};
pub use git::{InterdiffGit, Replay};
pub(crate) use record::{Recorded, record};
pub use store::InterdiffStore;

use crate::item::FileStat;

/// One file of the interdiff from an earlier revision to a later one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterdiffFile {
    /// `Changed`, `Added`, `Deleted` and `Dropped` files carry the diff from
    /// the earlier revision to the later; the others the later revision's own
    /// diff.
    pub stat: FileStat,
    pub change: Change,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// The later revision changes the file, differently than the earlier did.
    Changed,
    /// The later revision has the file, which the earlier didn't.
    Added,
    /// The later revision deletes the file, which the earlier had.
    Deleted,
    /// The later revision changes the file as the earlier did.
    Unchanged,
    /// The earlier revision's change conflicts with the target's when
    /// replayed onto the later base, so the two can't be told apart.
    Unseparated,
    /// The earlier revision changes the file; the later no longer does.
    Dropped,
}

impl Change {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::Added => "added",
            Self::Deleted => "deleted",
            Self::Unchanged => "unchanged",
            Self::Unseparated => "unseparated",
            Self::Dropped => "dropped",
        }
    }

    pub const ALL: [Self; 6] = [
        Self::Changed,
        Self::Added,
        Self::Deleted,
        Self::Unchanged,
        Self::Unseparated,
        Self::Dropped,
    ];
}

/// The interdiff from revision `from` to the one being recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInterdiff {
    pub from: u32,
    /// In path order.
    pub files: Vec<InterdiffFile>,
}
