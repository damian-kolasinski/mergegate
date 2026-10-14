//! Merging: an approved revision lands on its item's target as one commit
//! (design §5, F8, F9). A merge that can't land yet holds the item in
//! Merging until the operator retries or cancels it; one that conflicts hands
//! the item back to its agent.

mod approve;
mod backend;
mod blocked;
mod error;
mod hold;
mod land;

pub use approve::{Approve, approve};
pub use backend::{Landing, MergeBackend};
pub use blocked::{Blocked, blocked};
pub use error::MergeError;
pub use hold::{cancel, retry};

use crate::item::Item;

/// Where an approve or a retry left the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merge {
    pub item: Item,
    pub landing: Landing,
}
