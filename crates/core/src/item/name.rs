//! What the operator calls an item. On GitHub that's its pull request, as
//! in `app#84`, which the hub's own number (`app#3`) would only confuse;
//! elsewhere it's the item's ID.

use super::{Item, ItemBackend, ItemId, ItemStore};
use crate::store::StoreError;

impl Item {
    /// Its project and pull request on GitHub, else its ID.
    pub fn name(&self) -> ItemId {
        match &self.backend {
            ItemBackend::Github { pull, .. } => ItemId::new(self.id.project().clone(), pull.number),
            ItemBackend::Local => self.id.clone(),
        }
    }
}

/// The item `name` names, or whose ID it is. Names come first: they're what
/// agents and the operator are shown, while an ID that isn't also a name
/// still finds its item for a caller that kept one. A GitHub item's ID can
/// be another item's name, `app#5` naming pull request #5 while item `app#5`
/// is pull request #9, so a caller holding IDs it must not mistake should
/// look them up with [`ItemStore::item`].
///
/// A project that switched backends can have a local item and a GitHub one
/// with the same name; the open one wins, then the newest, as among pull
/// requests.
pub fn named(store: &impl ItemStore, name: &ItemId) -> Result<Option<Item>, StoreError> {
    let by_id = store.item(name)?;
    let local = by_id.as_ref().filter(|item| item.name() == *name).cloned();
    let pulled = store.item_with_pull(name.project(), name.number())?;
    let best = [pulled, local]
        .into_iter()
        .flatten()
        .max_by_key(|item| (item.state.is_open(), item.id.number()));
    Ok(best.or(by_id))
}

#[cfg(test)]
#[path = "name_tests.rs"]
mod tests;
