//! What a session's Stop hook watches between its turns: the items its
//! worktree owns and the ones it submitted from elsewhere, each with the
//! event worth waking the session for. One read answers for all of them, so
//! an idle session costs the hub a request per wait, however many items its
//! project has. Like the log, reading it hands nothing over.

use std::collections::BTreeSet;

use super::{FeedbackStore, attention};
use crate::agent::AgentStore;
use crate::item::{Item, ItemId, ItemStore};
use crate::store::StoreError;

/// The session a hook watches for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watch {
    /// The tailnet device the hook runs on; none on the hub's machine.
    pub device: Option<String>,
    /// The worktree the session ends its turns in, when it is a clone of a
    /// project. Its items are watched while they're open or closed today.
    pub worktree: Option<String>,
    /// Items watched wherever they are, each with the worktree the session
    /// sent it from, which its newest submit has to have come from still.
    pub items: Vec<(ItemId, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
    pub item: Item,
    /// Its newest unread event worth waking for, as [`attention`] finds it.
    pub cursor: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watching {
    /// Ordered by ID.
    pub items: Vec<Watched>,
    /// The newest cursor of the whole log, which the next read waits after.
    pub cursor: u64,
    /// A watched item has events after the cursor the read was given.
    pub changed: bool,
}

pub fn watch<S: ItemStore + FeedbackStore + AgentStore>(
    store: &S,
    watch: &Watch,
    since: Option<u64>,
) -> Result<Watching, StoreError> {
    let device = watch.device.as_deref();
    let mut ids = BTreeSet::new();
    if let Some(worktree) = &watch.worktree {
        ids.extend(store.watched_from(worktree, device)?);
    }
    for (id, worktree) in &watch.items {
        if store.agent(id)?.watched_from(worktree, device) {
            ids.insert(id.clone());
        }
    }
    let changed = match since {
        Some(since) => store.logged_since(since)?.iter().any(|id| ids.contains(id)),
        None => false,
    };
    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        let item = store
            .item(&id)?
            .ok_or_else(|| StoreError::new(format!("{id} has an agent but isn't stored")))?;
        let events = store.events(&id, store.delivered(&id)?)?;
        items.push(Watched {
            cursor: attention(item.state, &events),
            item,
        });
    }
    Ok(Watching {
        items,
        cursor: store.newest_cursor()?,
        changed,
    })
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
