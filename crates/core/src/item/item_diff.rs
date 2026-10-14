//! Opening an item for review: its newest revision with every file's diff.

use super::{ChangedFile, Item, ItemId, ItemStore};
use crate::diff::{DiffStore, FileDiff};
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDiff {
    pub item: Item,
    /// In path order.
    pub files: Vec<DiffedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffedFile {
    pub file: ChangedFile,
    pub diff: FileDiff,
}

/// `None` when there's no such item. Every diff was cached at submit, so this
/// only reads.
pub fn item_diff<S: ItemStore + DiffStore>(
    store: &S,
    id: &ItemId,
) -> Result<Option<ItemDiff>, StoreError> {
    let Some(item) = store.item(id)? else {
        return Ok(None);
    };
    let files = store
        .revision_files(id, item.revision.number)?
        .into_iter()
        .map(|file| {
            let diff = store
                .diff(file.old_blob.as_deref(), file.new_blob.as_deref())?
                .ok_or_else(|| {
                    StoreError::new(format!("{id} has no cached diff for {}", file.path))
                })?;
            Ok(DiffedFile { file, diff })
        })
        .collect::<Result<_, StoreError>>()?;
    Ok(Some(ItemDiff { item, files }))
}

#[cfg(test)]
#[path = "item_diff_tests.rs"]
mod tests;
