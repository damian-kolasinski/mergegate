//! Merging: an approved revision lands on its item's target as one commit
//! (design §5, F8, F9). A merge that can't land yet holds the item in
//! Merging until the operator retries or cancels it; one that conflicts hands
//! the item back to its agent.

mod approve;
mod backend;
mod blocked;
mod carry;
mod error;
mod git;
mod github;
mod hold;
mod land;
mod stacked;

pub use approve::{Approve, approve};
pub use backend::{Landing, MergeBackend};
pub use blocked::{Blocked, blocked};
pub use carry::{Carried, carry, standing_approval, unchanged_since_approval};
pub use error::MergeError;
pub use git::{MergeGit, Trial};
pub use github::GithubBackend;
pub use hold::{cancel, retry};
pub use stacked::merged_elsewhere;

use crate::item::Item;

/// Where an approve or a retry left the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merge {
    pub item: Item,
    pub landing: Landing,
    /// The items stacked on it that its merge moved, bottom first: merged
    /// right after it, or told to rebase.
    pub stacked: Vec<Item>,
}
