use super::{Item, ItemState, ItemStore};
use crate::store::StoreError;

/// Open items grouped by who acts next, each group oldest first.
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
    Ok(inbox)
}

#[cfg(test)]
#[path = "inbox_tests.rs"]
mod tests;
