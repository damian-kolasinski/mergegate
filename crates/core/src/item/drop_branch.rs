//! Dropping closed items' branches from staging (design §5).
//!
//! A closed item's branch would keep names that nest with it (`a/b` beside
//! `a`) from being pushed, as git refuses those, and nothing reads it once
//! the item is closed.

use super::{Item, ItemError, ItemGit, ItemId, ItemStore};
use crate::store::StoreError;

/// Drops `id`'s branch from staging if the item is closed. The branch stays
/// while an open item holds it, as one submitted from it again does, and
/// when it moved past the item's newest head, which is someone's work the
/// hub never saw.
pub fn drop_branch<S: ItemStore>(
    store: &S,
    git: &impl ItemGit,
    id: &ItemId,
) -> Result<(), ItemError> {
    let item = store
        .item(id)?
        .ok_or_else(|| ItemError::NoItem(id.clone()))?;
    drop_closed(store, git, &item)
}

/// Drops the branch of every closed item that still has it at its newest
/// head, as hubs that kept them left them, returning the items whose branch
/// couldn't be dropped and why.
pub fn drop_closed_branches<S: ItemStore>(
    store: &S,
    git: &impl ItemGit,
) -> Result<Vec<(ItemId, ItemError)>, StoreError> {
    let mut failed = Vec::new();
    for item in store.closed_items()? {
        if let Err(e) = drop_closed(store, git, &item) {
            failed.push((item.id, e));
        }
    }
    Ok(failed)
}

fn drop_closed<S: ItemStore>(store: &S, git: &impl ItemGit, item: &Item) -> Result<(), ItemError> {
    let project = item.id.project();
    // Once stacks land, a branch that open children target must stay too
    // (design §5.1).
    if item.state.is_open() || store.open_item(project, &item.branch)?.is_some() {
        return Ok(());
    }
    git.drop_branch(project, &item.branch, &item.revision.head)?;
    Ok(())
}

#[cfg(test)]
#[path = "drop_branch_tests.rs"]
mod tests;
