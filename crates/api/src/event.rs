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
    /// revision, which addresses replies.
    #[serde(rename = "item.changed")]
    ItemChanged {
        /// As in `app#42`.
        item: String,
    },
}
