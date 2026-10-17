//! How urgent an item is, and why. The authoring agent
//! passes both from its brief on the item's first submit; the operator and
//! their agent change them any time after.

use super::{Item, ItemError, ItemId, ItemStore};

/// p0 comes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
}

impl Priority {
    pub const ALL: [Self; 4] = [Self::P0, Self::P1, Self::P2, Self::P3];

    /// The name the store and the CLI spell it with.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::P0 => "p0",
            Self::P1 => "p1",
            Self::P2 => "p2",
            Self::P3 => "p3",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prioritize {
    pub id: ItemId,
    /// None clears it.
    pub priority: Option<Priority>,
    pub note: Option<String>,
}

/// Sets an open item's priority and note together. The latest write wins,
/// and one without a note clears the old note, which explained the old
/// priority. The item's agent isn't told: priority orders the operator's
/// queue, not the agent's work.
pub fn prioritize(store: &mut impl ItemStore, request: Prioritize) -> Result<Item, ItemError> {
    let Prioritize { id, priority, note } = request;
    let item = store
        .item(&id)?
        .ok_or_else(|| ItemError::NoItem(id.clone()))?;
    if !item.state.is_open() {
        return Err(ItemError::Closed(id));
    }
    let note = note_of(note);
    store.set_priority(&id, priority, note.as_deref())?;
    Ok(Item {
        priority,
        note,
        ..item
    })
}

/// A blank note is none.
pub(super) fn note_of(note: Option<String>) -> Option<String> {
    note.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty())
}

/// Where `priority` sorts: none after p3.
pub(super) fn rank(priority: Option<Priority>) -> usize {
    priority.map_or(Priority::ALL.len(), |p| p as usize)
}

#[cfg(test)]
#[path = "priority_tests.rs"]
mod tests;
