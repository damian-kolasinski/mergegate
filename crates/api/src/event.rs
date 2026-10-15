use serde::{Deserialize, Serialize};

/// One message on `GET /api/events/ui`, sent as the SSE `data` field. It
/// names what changed, and the UI refetches it; it carries no state itself.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UiEvent {
    #[serde(rename = "inbox.changed")]
    InboxChanged,
    /// The item's thread changed: its agent replied or submitted a new
    /// revision, which addresses replies. Also sent when the judge scored
    /// the newest revision's files.
    #[serde(rename = "item.changed")]
    ItemChanged {
        /// As in `app#42`.
        item: String,
    },
    /// Spans were computed, or a language pack installed or removed, so an
    /// open diff may show more highlighting, or less.
    #[serde(rename = "highlights.changed")]
    HighlightsChanged,
    /// The project's rules, or the changes to them waiting for the
    /// operator, changed.
    #[serde(rename = "rules.changed")]
    RulesChanged { project: String },
}
