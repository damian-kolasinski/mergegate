use std::cmp::Reverse;
use std::collections::BTreeMap;

use super::priority::rank;
use super::{Item, ItemId, ItemState, ItemStore};
use crate::review::{ReviewStore, Waiting, waiting};
use crate::store::StoreError;

/// Open items grouped by who acts next, each group in the order to act on
/// them: by priority, none last, then by how many open items are
/// stacked on each, most first, then oldest first. An item stacked on one in
/// its group follows it, so a stack reads bottom first. Then the items closed
/// today, most recently closed first.
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
    let above: BTreeMap<ItemId, usize> = open
        .iter()
        .map(|item| (item.id.clone(), stacked_above(&open, item)))
        .collect();
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
        group.sort_by_key(|item| {
            let above = above.get(&item.id).copied().unwrap_or_default();
            (rank(item.priority), Reverse(above))
        });
        *group = bottom_first(std::mem::take(group));
    }
    Ok(inbox)
}

/// How many open items are stacked on `item`, up its stack.
fn stacked_above(open: &[Item], item: &Item) -> usize {
    let mut count = 0;
    let mut below = &item.id;
    while let Some(child) = open
        .iter()
        .find(|o| o.parent.as_ref() == Some(below))
        .filter(|_| count < open.len())
    {
        count += 1;
        below = &child.id;
    }
    count
}

/// `group` with each item stacked on another of it moved right after that
/// one, the rest in their order.
fn bottom_first(group: Vec<Item>) -> Vec<Item> {
    let in_group = |id: &ItemId| group.iter().any(|item| item.id == *id);
    let mut ordered: Vec<Item> = Vec::with_capacity(group.len());
    for item in group
        .iter()
        .filter(|i| i.parent.as_ref().is_none_or(|p| !in_group(p)))
    {
        ordered.push(item.clone());
        while let Some(child) = group.iter().find(|child| {
            child.parent.as_ref() == ordered.last().map(|last| &last.id)
                && !ordered.iter().any(|o| o.id == child.id)
        }) {
            ordered.push(child.clone());
        }
    }
    // A cycle the store can't hold would leave items out; keep them.
    for item in &group {
        if !ordered.iter().any(|o| o.id == item.id) {
            ordered.push(item.clone());
        }
    }
    ordered
}

#[cfg(test)]
#[path = "inbox_tests.rs"]
mod tests;
