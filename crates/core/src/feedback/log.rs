//! The whole hub's event log, which `mg events --follow` streams to the
//! operator agent so it can start work that waited on an item (§6.2, F8).
//! Reading it acknowledges nothing: the log belongs to each item's agent.

use super::{Event, FeedbackStore};
use crate::item::{Item, ItemStore};
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Log {
    /// Oldest first, each with its item as it is now.
    pub events: Vec<(Item, Event)>,
    /// Where the next read starts: the newest event's cursor, or the one this
    /// read started from.
    pub cursor: u64,
}

/// Every item's events after `since`. Without it, none and the newest
/// cursor, so a stream starts from now.
pub fn log<S: ItemStore + FeedbackStore>(store: &S, since: Option<u64>) -> Result<Log, StoreError> {
    let Some(since) = since else {
        return Ok(Log {
            events: Vec::new(),
            cursor: store.newest_cursor()?,
        });
    };
    let mut events = Vec::new();
    for id in store.logged_since(since)? {
        let item = store
            .item(&id)?
            .ok_or_else(|| StoreError::new(format!("{id} has events but isn't stored")))?;
        events.extend(
            store
                .events(&id, since)?
                .into_iter()
                .map(|event| (item.clone(), event)),
        );
    }
    events.sort_by_key(|(_, event)| event.cursor);
    let cursor = events.last().map_or(since, |(_, event)| event.cursor);
    Ok(Log { events, cursor })
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
