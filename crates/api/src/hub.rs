use serde::{Deserialize, Serialize};

/// `GET /api/health`. Stable across API versions.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// Version of the `mg` binary running the hub.
    pub version: String,
    pub api_version: u32,
    pub schema_version: u32,
    pub pid: u32,
    /// The hub's `MERGEGATE_HOME`.
    pub home: String,
}
