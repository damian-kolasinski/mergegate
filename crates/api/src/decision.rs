use serde::{Deserialize, Serialize};

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
