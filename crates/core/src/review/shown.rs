//! The lines of a revision's file a comment can be on: the UI puts a comment
//! under its line, so one on a line the diff doesn't show, as in an expanded
//! gap, would be sent but never seen.

use super::Side;
use crate::diff::{Content, DiffStore, Hunk};
use crate::interdiff::{Change, InterdiffStore};
use crate::item::{ChangedFile, ItemId};
use crate::store::StoreError;

/// The hunks of one file of a revision that comments go on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Shown {
    /// The revision's own diff, on either side.
    own: Vec<Hunk>,
    /// On the new side only, every interdiff into the revision that changes
    /// or adds the file: its new side is the revision's file, so a comment
    /// there is on the revision as much as one on its own diff.
    since: Vec<Vec<Hunk>>,
}

impl Shown {
    /// `None` for a file that isn't text. Submitting a revision caches every
    /// file's diff, which calls a file binary as git does and shows it no
    /// lines.
    pub(crate) fn read<S: DiffStore + InterdiffStore>(
        store: &S,
        id: &ItemId,
        revision: u32,
        file: &ChangedFile,
    ) -> Result<Option<Self>, StoreError> {
        let diff = store
            .diff(file.old_blob.as_deref(), file.new_blob.as_deref())?
            .ok_or_else(|| StoreError::new(format!("{id} has no cached diff for {}", file.path)))?;
        let own = match diff.content {
            Content::Binary => return Ok(None),
            Content::Text(hunks) => hunks,
            Content::TooLarge => Vec::new(),
        };
        let mut since = Vec::new();
        for from in store.interdiffs_to(id, revision)? {
            let changed = store
                .interdiff(id, from, revision)?
                .into_iter()
                .flatten()
                .find(|f| {
                    f.stat.file.path == file.path
                        && matches!(f.change, Change::Changed | Change::Added)
                });
            let Some(changed) = changed else { continue };
            let file = changed.stat.file;
            let diff = store.diff(file.old_blob.as_deref(), file.new_blob.as_deref())?;
            if let Some(Content::Text(hunks)) = diff.map(|diff| diff.content) {
                since.push(hunks);
            }
        }
        Ok(Some(Self { own, since }))
    }

    pub(crate) fn shows(&self, side: Side, start_line: u32, line: u32) -> bool {
        shown(&self.own, side, start_line, line)
            || (side == Side::New
                && self
                    .since
                    .iter()
                    .any(|hunks| shown(hunks, side, start_line, line)))
    }
}

/// Whether one hunk shows lines `start_line..=line` of `side`.
pub(crate) fn shown(hunks: &[Hunk], side: Side, start_line: u32, line: u32) -> bool {
    hunks.iter().any(|hunk| {
        let (first, count) = match side {
            Side::Old => (hunk.old_start, hunk.old_lines),
            Side::New => (hunk.new_start, hunk.new_lines),
        };
        // A side without lines starts at the line before the hunk.
        count > 0 && first <= start_line && start_line <= line && line < first + count
    })
}

#[cfg(test)]
#[path = "shown_tests.rs"]
mod tests;
