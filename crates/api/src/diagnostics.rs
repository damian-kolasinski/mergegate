use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub timestamp: u64,
    pub run_id: String,
    #[serde(default)]
    pub version: String,
    /// Sanitized free-form warning/error text; extraction requires opt-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub sequence: u64,
    pub level: String,
    pub target: String,
    pub line: Option<u32>,
    pub fields: BTreeMap<String, String>,
    pub dropped: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub format_version: u32,
    pub captured_at: u64,
    pub hub_version: Option<String>,
    pub api_version: Option<u32>,
    pub schema_version: Option<u32>,
    pub records: Vec<Record>,
    pub warnings: Vec<String>,
}
