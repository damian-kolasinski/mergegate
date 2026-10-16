use super::{FileStat, Item, ItemBackend, ItemId, ItemState, Priority, Revision};
use crate::interdiff::NewInterdiff;
use crate::project::ProjectName;
use crate::store::StoreError;

/// Items and their revisions. The store stamps creation and submit times.
pub trait ItemStore {
    /// Open items, oldest first; only `project`'s when given.
    fn open_items(&self, project: Option<&ProjectName>) -> Result<Vec<Item>, StoreError>;

    /// The project's merged items, newest first, at most `limit`.
    fn merged_items(&self, project: &ProjectName, limit: u32) -> Result<Vec<Item>, StoreError>;

    /// Merged and abandoned items of every project, oldest first.
    fn closed_items(&self) -> Result<Vec<Item>, StoreError>;

    /// Items merged or abandoned since local midnight, the latest closed
    /// first.
    fn closed_today(&self) -> Result<Vec<Item>, StoreError>;

    fn open_item(&self, project: &ProjectName, branch: &str) -> Result<Option<Item>, StoreError>;

    /// Any item, open or closed.
    fn item(&self, id: &ItemId) -> Result<Option<Item>, StoreError>;

    /// The project's item with pull request `pull` on GitHub. A project that
    /// moved to another repository may have one on each; the open one wins,
    /// then the newest.
    fn item_with_pull(&self, project: &ProjectName, pull: u64) -> Result<Option<Item>, StoreError>;

    /// The open item stacked on `id`, of which there's at most one (§5.1).
    fn child(&self, id: &ItemId) -> Result<Option<Item>, StoreError>;

    /// Every revision of `id`, oldest first.
    fn revisions(&self, id: &ItemId) -> Result<Vec<Revision>, StoreError>;

    /// The files `revision` changes, in path order, with their line counts.
    fn revision_files(&self, id: &ItemId, revision: u32) -> Result<Vec<FileStat>, StoreError>;

    /// The ID [`Self::insert_item`] gives the project's next item, until
    /// another is inserted.
    fn next_id(&self, project: &ProjectName) -> Result<ItemId, StoreError>;

    /// Numbers the item next in its project.
    fn insert_item(&mut self, item: NewItem) -> Result<Item, StoreError>;

    /// Records the revision with its interdiffs and carried viewed marks, all
    /// or nothing, and stacks the item on the revision's parent.
    fn add_revision(
        &mut self,
        id: &ItemId,
        title: &str,
        state: ItemState,
        revision: NewRevision,
    ) -> Result<Item, StoreError>;

    /// The agent's review guide for `revision`, if it sent one.
    fn guide(&self, id: &ItemId, revision: u32) -> Result<Option<String>, StoreError>;

    fn set_guide(
        &mut self,
        id: &ItemId,
        revision: u32,
        guide: Option<&str>,
    ) -> Result<(), StoreError>;

    fn set_title(&mut self, id: &ItemId, title: &str) -> Result<(), StoreError>;

    fn set_priority(
        &mut self,
        id: &ItemId,
        priority: Option<Priority>,
        note: Option<&str>,
    ) -> Result<(), StoreError>;
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
    pub priority: Option<Priority>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRevision {
    pub head: String,
    pub base: String,
    pub files: Vec<FileStat>,
    pub guide: Option<String>,
    /// From each earlier revision; none for an item's first.
    pub interdiffs: Vec<NewInterdiff>,
    /// Files that start out viewed.
    pub viewed: Vec<String>,
    /// The item it's stacked on from this revision on; none unstacks it.
    pub parent: Option<ItemId>,
}
