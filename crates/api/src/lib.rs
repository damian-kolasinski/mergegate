//! Wire types shared by the CLI, the hub and (later) the UI.

use serde::{Deserialize, Serialize};

/// Version of the HTTP API between `mg` and the hub. A CLI refuses to talk to
/// a hub with a different API version, except through the endpoints below that
/// every version keeps: `GET /api/health` and `POST /api/hub/shutdown`.
pub const API_VERSION: u32 = 1;

/// Header every mutating request must carry. Browsers can't add it to a
/// cross-origin request without a CORS preflight, which the hub never grants.
pub const CLIENT_HEADER: &str = "x-mergegate-client";

/// `GET /api/health`. Stable across API versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// Version of the `mg` binary running the hub.
    pub version: String,
    pub api_version: u32,
    pub schema_version: u32,
    pub pid: u32,
    /// The hub's `MERGEGATE_HOME`, so a client can tell it reached its own hub.
    pub home: String,
}

/// An error, as the hub returns it over HTTP and `mg --json` prints it:
/// `{"error": {...}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Stable, machine-readable code, e.g. `port_in_use`.
    pub code: String,
    pub message: String,
    /// The command or action that fixes the error, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

/// A decision the operator has to make before a command can proceed:
/// `{"needs_decision": {...}}`. The agent asks the operator and re-runs the
/// command with `flag` set to the chosen option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedsDecision {
    pub question: String,
    pub flag: String,
    pub options: Vec<DecisionOption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionOption {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedsDecisionEnvelope {
    pub needs_decision: NeedsDecision,
}
