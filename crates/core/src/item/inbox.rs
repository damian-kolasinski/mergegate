use super::priority::rank;
use super::{Item, ItemState, ItemStore};
use crate::store::StoreError;

/// Open items grouped by who acts next, each group in the order to act on
/// them (§9.1): by priority, none last, then oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inbox {
    pub needs_you: Vec<Item>,
    pub with_agents: Vec<Item>,
    pub merging: Vec<Item>,
}

pub fn inbox(store: &impl ItemStore) -> Result<Inbox, StoreError> {
    let mut inbox = Inbox::default();
    for item in store.open_items(None)? {
        let group = match item.state {
            ItemState::NeedsReview => &mut inbox.needs_you,
            ItemState::ChangesRequested => &mut inbox.with_agents,
            ItemState::Merging => &mut inbox.merging,
            ItemState::Merged | ItemState::Abandoned => continue,
        };
        group.push(item);
    }
    // Stable, so items of one priority keep the store's oldest-first order.
    for group in [
        &mut inbox.needs_you,
        &mut inbox.with_agents,
        &mut inbox.merging,
    ] {
        group.sort_by_key(|item| rank(item.priority));
    }
    Ok(inbox)
}

#[cfg(test)]
#[path = "inbox_tests.rs"]
mod tests;
