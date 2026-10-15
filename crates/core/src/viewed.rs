//! Viewed marks: the files the reviewer is done with. A mark holds for the
//! revision it was made on, and every later one the file is unchanged in.

use std::collections::BTreeSet;

use crate::interdiff::{Change, InterdiffStore};
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

/// Marks or unmarks one of the files `mark.revision` changes, and the same
/// file in each later revision that changes it as `mark.revision` does.
pub fn mark<S: ItemStore + ViewedStore + InterdiffStore>(
    store: &mut S,
    mark: Mark,
) -> Result<(), ViewedError> {
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
        .any(|stat| stat.file.path == path)
    {
        return Err(ViewedError::NoFile { id, revision, path });
    }
    store.set_viewed(&id, revision, &path, viewed)?;
    for later in revision + 1..=item.revision.number {
        let unchanged = store
            .interdiff(&id, revision, later)?
            .unwrap_or_default()
            .iter()
            .any(|f| f.stat.file.path == path && f.change == Change::Unchanged);
        if unchanged {
            store.set_viewed(&id, later, &path, viewed)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "viewed_tests.rs"]
mod tests;
