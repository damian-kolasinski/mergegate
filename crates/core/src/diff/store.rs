use super::FileDiff;
use crate::item::ItemId;
use crate::store::StoreError;

/// The diff cache, keyed by the old and new blob IDs, with `None` for the side
/// where the file doesn't exist. A blob pair's diff never changes, so an entry
/// is only ever added, or dropped once no open item needs it.
pub trait DiffStore {
    fn diff(&self, old: Option<&str>, new: Option<&str>) -> Result<Option<FileDiff>, StoreError>;

    /// Replaces an existing entry, which holds the same diff.
    fn insert_diff(
        &mut self,
        old: Option<&str>,
        new: Option<&str>,
        diff: &FileDiff,
    ) -> Result<(), StoreError>;

    /// Drops the entries of `id`'s files, in any revision, that no revision
    /// of an open item has.
    fn prune_diffs(&mut self, id: &ItemId) -> Result<(), StoreError>;
}
