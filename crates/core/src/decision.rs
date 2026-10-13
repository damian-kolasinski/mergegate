//! Decisions only the operator can make. A command that needs one changes
//! nothing and returns it; the agent asks the operator and re-runs the
//! command with `flag` set to the chosen option.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub question: String,
    pub flag: String,
    pub options: Vec<DecisionOption>,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionOption {
    pub value: String,
    pub description: Option<String>,
}
