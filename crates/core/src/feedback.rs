//! The event log: what an item's agent has to react to, read with `mg wait`
//! and `mg feedback` (design §6.1, §6.3). Each item keeps the cursor of the
//! newest event its agent acknowledged, so a wait that dropped before the
//! agent saw its events returns them again.

use crate::item::{Item, ItemId, ItemState, ItemStore};
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

    /// Moves `id` to `state` and appends `notice`, all or nothing: an agent
    /// never misses a change to its item that the operator saw happen.
    fn notify(
        &mut self,
        id: &ItemId,
        state: ItemState,
        notice: Notice,
    ) -> Result<Event, StoreError>;
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
    Notice(Notice),
}

/// Something that happened to the item itself, which the event is the only
/// record of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The item's newest revision when it happened.
    pub revision: u32,
    pub source: Source,
    pub kind: NoticeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeKind {
    /// `reason` is none when whoever abandoned it gave none.
    Abandoned { reason: Option<String> },
    /// The approved revision didn't land and waits for the operator to retry,
    /// as when their checkout of the target is dirty; `reason` is git's.
    MergeHeld { reason: String },
    /// The operator gave up on a held merge; the item needs review again.
    MergeCancelled,
    /// The approved revision conflicts with the target at `target_commit`.
    MergeConflict {
        target_commit: String,
        paths: Vec<String>,
    },
    /// The approved revision landed as `commit` on the target.
    Merged { commit: String },
}

/// Who an event comes from. External reviewers join in PR 10.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Operator,
    /// The item's own agent, through `mg`.
    Agent,
}

impl Source {
    pub const ALL: [Self; 2] = [Self::Operator, Self::Agent];

    /// The name the store spells the source with.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Agent => "agent",
        }
    }
}

impl Event {
    pub fn source(&self) -> Source {
        match &self.kind {
            EventKind::ReviewSubmitted(_) => Source::Operator,
            EventKind::Notice(notice) => notice.source,
        }
    }

    /// The revision the event is about.
    pub fn revision(&self) -> u32 {
        match &self.kind {
            EventKind::ReviewSubmitted(review) => review.revision,
            EventKind::Notice(notice) => notice.revision,
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
