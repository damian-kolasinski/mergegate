use super::{FileStat, Item, ItemId, ItemState};
use crate::project::{Backend, ProjectName};
use crate::store::StoreError;

/// Items and their revisions. The store stamps creation and submit times.
pub trait ItemStore {
    /// Open items, oldest first; only `project`'s when given.
    fn open_items(&self, project: Option<&ProjectName>) -> Result<Vec<Item>, StoreError>;

    /// The open item for `branch` in `project`, if there is one.
    fn open_item(&self, project: &ProjectName, branch: &str) -> Result<Option<Item>, StoreError>;

    /// Records a new item, numbered next in its project, with its first
    /// revision.
    fn insert_item(&mut self, item: NewItem) -> Result<Item, StoreError>;

    /// Records `revision` as the item's newest, sets its title and moves it to
    /// `state`.
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
    pub revision: NewRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRevision {
    pub head: String,
    pub base: String,
    pub files: Vec<FileStat>,
}
