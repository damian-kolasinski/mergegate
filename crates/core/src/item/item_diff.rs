//! Opening an item for review: its newest revision with every file's diff.

use super::{ChangedFile, Item, ItemId, ItemStore};
use crate::diff::{DiffStore, FileDiff};
use crate::review::{Comment, Reply, ReviewStore};
use crate::store::StoreError;
use crate::viewed::ViewedStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDiff {
    pub item: Item,
    /// In path order.
    pub files: Vec<DiffedFile>,
    /// On every revision, drafts and sent ones, oldest first.
    pub comments: Vec<Comment>,
    /// On every comment, oldest first.
    pub replies: Vec<Reply>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffedFile {
    pub file: ChangedFile,
    pub diff: FileDiff,
    /// Marked viewed in this revision.
    pub viewed: bool,
}

/// `None` when there's no such item. Every diff was cached at submit, so this
/// only reads.
pub fn item_diff<S: ItemStore + DiffStore + ViewedStore + ReviewStore>(
    store: &S,
    id: &ItemId,
) -> Result<Option<ItemDiff>, StoreError> {
    let Some(item) = store.item(id)? else {
        return Ok(None);
    };
    let viewed = store.viewed(id, item.revision.number)?;
    let files = store
        .revision_files(id, item.revision.number)?
        .into_iter()
        .map(|file| {
            let diff = store
                .diff(file.old_blob.as_deref(), file.new_blob.as_deref())?
                .ok_or_else(|| {
                    StoreError::new(format!("{id} has no cached diff for {}", file.path))
                })?;
            let viewed = viewed.contains(&file.path);
            Ok(DiffedFile { file, diff, viewed })
        })
        .collect::<Result<_, StoreError>>()?;
    let comments = store.comments(id)?;
    let replies = store.replies(id)?;
    Ok(Some(ItemDiff {
        item,
        files,
        comments,
        replies,
    }))
}

#[cfg(test)]
#[path = "item_diff_tests.rs"]
mod tests;
