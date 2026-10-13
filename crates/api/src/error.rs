use serde::{Deserialize, Serialize};

/// An error, as the hub returns it over HTTP and `mg --json` prints it:
/// `{"error": {...}}`.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Stable, machine-readable code, e.g. `port_in_use`.
    pub code: String,
    pub message: String,
    /// The command or action that fixes the error, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}
