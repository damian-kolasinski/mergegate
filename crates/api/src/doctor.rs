use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

/// One thing `mg doctor` verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    /// Stable, machine-readable name, e.g. `git` or `project app`.
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    /// The command that fixes a `warn` or `fail`, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// `GET /api/doctor`: the checks only the hub can run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub checks: Vec<Check>,
}
