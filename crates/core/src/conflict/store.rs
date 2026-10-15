use crate::item::ItemId;
use crate::project::ProjectName;
use crate::store::StoreError;

/// Whether two items of a project merge into each other cleanly, as of
/// these heads. `item` is numbered below `other`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub item: ItemId,
    pub item_head: String,
    pub other: ItemId,
    pub other_head: String,
    pub conflicts: bool,
}

/// The pairs last tried. They stand until the next try, even once an item
/// has moved on or closed, so readers check the heads.
pub trait ConflictStore {
    /// Every project's, or only `project`'s, by `item` then `other`.
    fn pairs(&self, project: Option<&ProjectName>) -> Result<Vec<Pair>, StoreError>;

    /// Replaces `project`'s pairs.
    fn set_pairs(&mut self, project: &ProjectName, pairs: &[Pair]) -> Result<(), StoreError>;
}
