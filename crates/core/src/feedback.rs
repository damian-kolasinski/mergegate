//! The event log: what an item's agent has to react to, read with `mg wait`
//! and `mg feedback` (design §6.1, §6.3). Each item keeps the cursor of the
//! newest event its agent acknowledged, so a wait that dropped before the
//! agent saw its events returns them again.

use crate::item::{Item, ItemId, ItemStore};
use crate::review::Review;
use crate::store::StoreError;

pub trait FeedbackStore {
    /// `id`'s events after `cursor`, oldest first.
    fn events(&self, id: &ItemId, after: u64) -> Result<Vec<Event>, StoreError>;

    /// The cursor of `id`'s newest event, 0 when it has none.
    fn last_cursor(&self, id: &ItemId) -> Result<u64, StoreError>;

    /// The cursor `id`'s agent acknowledged last, 0 before the first.
    fn delivered(&self, id: &ItemId) -> Result<u64, StoreError>;

    fn set_delivered(&mut self, id: &ItemId, cursor: u64) -> Result<(), StoreError>;
}

/// One entry of the log. Cursors are hub-wide, so an item's events have
/// increasing but not consecutive ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub cursor: u64,
    pub kind: EventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    ReviewSubmitted(Review),
}

/// Who an event comes from. External reviewers join in PR 10.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Operator,
}

impl Event {
    pub fn source(&self) -> Source {
        match self.kind {
            EventKind::ReviewSubmitted(_) => Source::Operator,
        }
    }

    /// The revision the event is about.
    pub fn revision(&self) -> u32 {
        match &self.kind {
            EventKind::ReviewSubmitted(review) => review.revision,
        }
    }
}

/// Which events end a wait. It returns every undelivered event either way, so
/// acknowledging them never skips one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wake {
    #[default]
    Any,
    Operator,
}

impl Wake {
    pub fn by(self, events: &[Event]) -> bool {
        match self {
            Self::Any => !events.is_empty(),
            Self::Operator => events.iter().any(|e| e.source() == Source::Operator),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feedback {
    pub item: Item,
    pub events: Vec<Event>,
    /// Where the next read starts: the newest event's cursor, or the one this
    /// read started from.
    pub cursor: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum FeedbackError {
    #[error("no item {0}")]
    NoItem(ItemId),
    #[error("{id} has no event {cursor}")]
    NoEvent { id: ItemId, cursor: u64 },
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// `id`'s events after `since`, or after the delivered cursor.
pub fn feedback<S: ItemStore + FeedbackStore>(
    store: &S,
    id: &ItemId,
    since: Option<u64>,
) -> Result<Feedback, FeedbackError> {
    let item = store
        .item(id)?
        .ok_or_else(|| FeedbackError::NoItem(id.clone()))?;
    let since = match since {
        Some(cursor) => cursor,
        None => store.delivered(id)?,
    };
    let events = store.events(id, since)?;
    let cursor = events.last().map_or(since, |e| e.cursor);
    Ok(Feedback {
        item,
        events,
        cursor,
    })
}

/// Marks `id`'s events up to `cursor` delivered. Acknowledging an older
/// cursor changes nothing, so a replay with `--since` never re-delivers.
pub fn acknowledge<S: ItemStore + FeedbackStore>(
    store: &mut S,
    id: &ItemId,
    cursor: u64,
) -> Result<u64, FeedbackError> {
    if store.item(id)?.is_none() {
        return Err(FeedbackError::NoItem(id.clone()));
    }
    // A cursor past the newest event would hide the next one.
    if cursor > store.last_cursor(id)? {
        return Err(FeedbackError::NoEvent {
            id: id.clone(),
            cursor,
        });
    }
    let delivered = store.delivered(id)?;
    if cursor <= delivered {
        return Ok(delivered);
    }
    store.set_delivered(id, cursor)?;
    Ok(cursor)
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
