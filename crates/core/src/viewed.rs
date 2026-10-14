//! Viewed marks: the files the reviewer is done with. A mark holds for the
//! revision it was made on.

use std::collections::BTreeSet;

use crate::item::{ItemId, ItemStore};
use crate::store::StoreError;

pub trait ViewedStore {
    /// The paths marked viewed in `revision`.
    fn viewed(&self, id: &ItemId, revision: u32) -> Result<BTreeSet<String>, StoreError>;

    /// Marking a viewed file again, or unmarking one that isn't, changes
    /// nothing.
    fn set_viewed(
        &mut self,
        id: &ItemId,
        revision: u32,
        path: &str,
        viewed: bool,
    ) -> Result<(), StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    pub id: ItemId,
    pub revision: u32,
    pub path: String,
    pub viewed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ViewedError {
    #[error("no item {0}")]
    NoItem(ItemId),
    #[error("{id} has no revision {revision}")]
    NoRevision { id: ItemId, revision: u32 },
    #[error("{id} r{revision} doesn't change {path}")]
    NoFile {
        id: ItemId,
        revision: u32,
        path: String,
    },
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Marks or unmarks one of the files `mark.revision` changes.
pub fn mark<S: ItemStore + ViewedStore>(store: &mut S, mark: Mark) -> Result<(), ViewedError> {
    let Mark {
        id,
        revision,
        path,
        viewed,
    } = mark;
    let item = store
        .item(&id)?
        .ok_or_else(|| ViewedError::NoItem(id.clone()))?;
    if revision == 0 || revision > item.revision.number {
        return Err(ViewedError::NoRevision { id, revision });
    }
    if !store
        .revision_files(&id, revision)?
        .iter()
        .any(|file| file.path == path)
    {
        return Err(ViewedError::NoFile { id, revision, path });
    }
    store.set_viewed(&id, revision, &path, viewed)?;
    Ok(())
}

#[cfg(test)]
#[path = "viewed_tests.rs"]
mod tests;
