use std::cmp::Reverse;
use std::collections::BTreeMap;

use super::priority::rank;
use super::{Item, ItemId, ItemState, ItemStore};
use crate::conflict::{ConflictStore, conflicting};
use crate::store::StoreError;

/// Open items grouped by who acts next, each group in the order to act on
/// them (§9.1): by priority, none last, then by how many open items each
/// conflicts with, most first, then oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inbox {
    pub needs_you: Vec<Item>,
    pub with_agents: Vec<Item>,
    pub merging: Vec<Item>,
    /// The open items each item's head conflicts with; none isn't listed.
    pub conflicts: BTreeMap<ItemId, Vec<ItemId>>,
}

pub fn inbox(store: &(impl ItemStore + ConflictStore)) -> Result<Inbox, StoreError> {
    let open = store.open_items(None)?;
    let mut inbox = Inbox {
        conflicts: conflicting(&open, &store.pairs(None)?),
        ..Inbox::default()
    };
    for item in open {
        let group = match item.state {
            ItemState::NeedsReview => &mut inbox.needs_you,
            ItemState::ChangesRequested => &mut inbox.with_agents,
            ItemState::Merging => &mut inbox.merging,
            ItemState::Merged | ItemState::Abandoned => continue,
        };
        group.push(item);
    }
    let pressure = |item: &Item| inbox.conflicts.get(&item.id).map_or(0, Vec::len);
    // Stable, so items that tie keep the store's oldest-first order.
    for group in [
        &mut inbox.needs_you,
        &mut inbox.with_agents,
        &mut inbox.merging,
    ] {
        group.sort_by_key(|item| (rank(item.priority), Reverse(pressure(item))));
    }
    Ok(inbox)
}

#[cfg(test)]
#[path = "inbox_tests.rs"]
mod tests;
