use super::InterdiffFile;
use crate::item::ItemId;
use crate::store::StoreError;

/// Interdiffs recorded at submit. They're written with the revision they
/// lead to (`NewRevision::interdiffs`), so this only reads.
pub trait InterdiffStore {
    /// The interdiff from revision `from` to `to`, in path order; none when it
    /// wasn't recorded, as for an item submitted before interdiffs were.
    fn interdiff(
        &self,
        id: &ItemId,
        from: u32,
        to: u32,
    ) -> Result<Option<Vec<InterdiffFile>>, StoreError>;

    /// The revisions `to` has an interdiff from, ascending.
    fn interdiffs_to(&self, id: &ItemId, to: u32) -> Result<Vec<u32>, StoreError>;
}
