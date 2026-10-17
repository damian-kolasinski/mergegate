use serde::{Deserialize, Serialize};

/// Whether what the operator sends an open item reaches an agent.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPresence {
    /// An `mg wait` or `mg feedback` on the item is reading, or just did.
    Waiting,
    /// Back with its agent, who has read everything sent to it.
    Working,
    /// Nothing reads it, but the hub runs the item's wake command.
    Wakes,
    /// Nothing reads it and nothing wakes one: what the operator sends
    /// waits until a session picks the item up with `mg resume`.
    Away,
}
