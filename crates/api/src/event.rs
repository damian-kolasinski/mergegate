use serde::{Deserialize, Serialize};

/// One message on `GET /api/events/ui`, sent as the SSE `data` field. It
/// names what changed, and the UI refetches it; it carries no state itself.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UiEvent {
    #[serde(rename = "inbox.changed")]
    InboxChanged,
}
