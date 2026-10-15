use super::{FileStat, Item, ItemBackend, ItemId, ItemState, Revision};
use crate::project::ProjectName;
use crate::store::StoreError;

/// Items and their revisions. The store stamps creation and submit times.
pub trait ItemStore {
    /// Open items, oldest first; only `project`'s when given.
    fn open_items(&self, project: Option<&ProjectName>) -> Result<Vec<Item>, StoreError>;

    /// The project's merged items, newest first, at most `limit`.
    fn merged_items(&self, project: &ProjectName, limit: u32) -> Result<Vec<Item>, StoreError>;

    fn open_item(&self, project: &ProjectName, branch: &str) -> Result<Option<Item>, StoreError>;

    /// Any item, open or closed.
    fn item(&self, id: &ItemId) -> Result<Option<Item>, StoreError>;

    /// Every revision of `id`, oldest first.
    fn revisions(&self, id: &ItemId) -> Result<Vec<Revision>, StoreError>;

    /// The files `revision` changes, in path order, with their line counts.
    fn revision_files(&self, id: &ItemId, revision: u32) -> Result<Vec<FileStat>, StoreError>;

    /// Numbers the item next in its project.
    fn insert_item(&mut self, item: NewItem) -> Result<Item, StoreError>;

    fn add_revision(
        &mut self,
        id: &ItemId,
        title: &str,
        state: ItemState,
        revision: NewRevision,
    ) -> Result<Item, StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItem {
    pub project: ProjectName,
    pub branch: String,
    pub title: String,
    pub state: ItemState,
    pub backend: ItemBackend,
    pub target: String,
    pub revision: NewRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRevision {
    pub head: String,
    pub base: String,
    pub files: Vec<FileStat>,
}
