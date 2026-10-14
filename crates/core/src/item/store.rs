use super::{FileStat, Item, ItemId, ItemState};
use crate::github::Pull;
use crate::project::{Backend, ProjectName};
use crate::store::StoreError;

/// Items and their revisions. The store stamps creation and submit times.
pub trait ItemStore {
    /// Open items, oldest first; only `project`'s when given.
    fn open_items(&self, project: Option<&ProjectName>) -> Result<Vec<Item>, StoreError>;

    fn open_item(&self, project: &ProjectName, branch: &str) -> Result<Option<Item>, StoreError>;

    /// Any item, open or closed.
    fn item(&self, id: &ItemId) -> Result<Option<Item>, StoreError>;

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
    pub backend: Backend,
    pub target: String,
    pub pull: Option<Pull>,
    pub revision: NewRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRevision {
    pub head: String,
    pub base: String,
    pub files: Vec<FileStat>,
}
