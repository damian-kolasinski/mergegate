use std::collections::BTreeMap;

use super::priority::rank;
use super::{Item, ItemId, ItemState, ItemStore};
use crate::review::{ReviewStore, Waiting, waiting};
use crate::store::StoreError;

/// Open items grouped by who acts next, each group in the order to act on
/// them (§9.1): by priority, none last, then oldest first. Then the items
/// closed today, most recently closed first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inbox {
    pub needs_you: Vec<Item>,
    pub with_agents: Vec<Item>,
    pub merging: Vec<Item>,
    pub closed: Vec<Item>,
    /// What each open item's agent waits on the operator for; nothing isn't
    /// listed.
    pub waiting: BTreeMap<ItemId, Waiting>,
}

pub fn inbox(store: &(impl ItemStore + ReviewStore)) -> Result<Inbox, StoreError> {
    let open = store.open_items(None)?;
    let mut inbox = Inbox {
        closed: store.closed_today()?,
        ..Inbox::default()
    };
    for item in open {
        let owed = waiting(&store.comments(&item.id)?, &store.replies(&item.id)?);
        if owed.any() {
            inbox.waiting.insert(item.id.clone(), owed);
        }
        let group = match item.state {
            ItemState::NeedsReview => &mut inbox.needs_you,
            // An agent that asked something can't go on until it hears back.
            ItemState::ChangesRequested if owed.any() => &mut inbox.needs_you,
            ItemState::ChangesRequested => &mut inbox.with_agents,
            ItemState::Merging => &mut inbox.merging,
            ItemState::Merged | ItemState::Abandoned => continue,
        };
        group.push(item);
    }
    // Stable, so items that tie keep the store's oldest-first order.
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
