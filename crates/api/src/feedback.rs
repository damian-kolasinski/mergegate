//! What `mg feedback` reads: an item's events (design §6.3).

use serde::{Deserialize, Serialize};

use crate::item::ItemState;
use crate::review::Verdict;

/// `GET /api/items/{id}/feedback`: the events after `since`, or after the
/// item's delivered cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    pub item: String,
    pub state: ItemState,
    /// Oldest first.
    pub events: Vec<Event>,
    /// Where the next read starts; acknowledge it once the events are handled.
    pub cursor: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub cursor: u64,
    pub item: String,
    /// The revision the event is about.
    pub revision: u32,
    pub source: Source,
    #[serde(flatten)]
    pub body: EventBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Operator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventBody {
    #[serde(rename = "review.submitted")]
    ReviewSubmitted { verdict: Verdict, summary: String },
}

/// `POST /api/items/{id}/feedback/ack`: the events up to `cursor` were
/// handled. Answered with [`Acknowledged`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acknowledge {
    pub cursor: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acknowledged {
    /// The item's delivered cursor now, which an older `cursor` doesn't move.
    pub delivered: u64,
}
