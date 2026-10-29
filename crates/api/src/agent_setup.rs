//! The skill stub and session hook in each coding agent on the hub's
//! machine: `mg skill status` and `setup`, and the UI's prompt to set agents
//! up, through `/api/agents`.

use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentId {
    Claude,
    Codex,
    OpenCode,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SetupKind {
    Skill,
    Hook,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SetupState {
    /// Installed as this `mg` installs it.
    Current,
    /// Installed by another `mg`, or changed since.
    Outdated,
    Missing,
    /// A skill named mergegate that no `mg` wrote.
    Foreign,
    Unreadable,
    /// The agent can't hold it, as one without command hooks a hook.
    Unsupported,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub state: SetupState,
    /// Why it is unreadable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSetup {
    pub agent: AgentId,
    /// As in `Claude Code`.
    pub label: String,
    /// Installed on the machine.
    pub found: bool,
    /// Setup has nothing left to do for it.
    pub ready: bool,
    pub skill: SetupItem,
    pub hook: SetupItem,
}

/// `GET /api/agents`, and `mg skill status`.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentsStatus {
    pub agents: Vec<AgentSetup>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SetupChange {
    Installed,
    Updated,
    Unchanged,
    Removed,
    /// Nothing of ours was there to remove.
    Absent,
    /// A skill named mergegate that no `mg` wrote, left as it was.
    Conflict,
    /// The same, which removing leaves alone.
    Kept,
    Unsupported,
    Failed,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupStep {
    /// The agents that read the item; agents sharing a folder share a step.
    pub agents: Vec<AgentId>,
    pub kind: SetupKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub change: SetupChange,
    /// Why it failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// What the operator does before a hook it wrote runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approve: Option<String>,
}

/// `POST /api/agents/setup`, and `mg skill setup` and `remove`.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupReport {
    pub dry_run: bool,
    pub steps: Vec<SetupStep>,
}
