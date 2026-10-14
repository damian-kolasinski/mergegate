//! What an item's agent leaves the hub when it submits: the worktree it works
//! in, which `mg resume` names, and the command that wakes it when the item
//! has events (design §6.5).

use crate::item::ItemId;
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Agent {
    /// The absolute path of the worktree the newest submit came from.
    pub worktree: Option<String>,
    /// A shell command the hub runs when the item has events from the
    /// operator its agent hasn't read.
    pub wake: Option<String>,
}

/// An item whose agent is to be woken: it has a wake command and an event
/// from the operator newer than both the one its agent acknowledged and the
/// one that woke it last. The agent's own events, like its `mg abandon`,
/// never wake it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueWake {
    pub item: ItemId,
    pub command: String,
    pub worktree: Option<String>,
    /// The item's newest event from the operator.
    pub cursor: u64,
}

pub trait AgentStore {
    /// Default when the item's agent never left anything.
    fn agent(&self, id: &ItemId) -> Result<Agent, StoreError>;

    fn set_agent(&mut self, id: &ItemId, agent: &Agent) -> Result<(), StoreError>;

    /// Every item's, open or closed: closing logs an event its agent reads.
    fn due_wakes(&self) -> Result<Vec<DueWake>, StoreError>;

    /// The agent was woken for, or has read, the events up to `cursor`.
    fn set_woken(&mut self, id: &ItemId, cursor: u64) -> Result<(), StoreError>;
}

/// Records what a submit carried. A submit without a worktree or wake keeps
/// the item's; an empty wake removes it.
pub(crate) fn record<S: AgentStore>(
    store: &mut S,
    id: &ItemId,
    worktree: Option<String>,
    wake: Option<String>,
) -> Result<(), StoreError> {
    if worktree.is_none() && wake.is_none() {
        return Ok(());
    }
    let mut agent = store.agent(id)?;
    if worktree.is_some() {
        agent.worktree = worktree;
    }
    if let Some(wake) = wake {
        let wake = wake.trim();
        agent.wake = (!wake.is_empty()).then(|| wake.to_owned());
    }
    store.set_agent(id, &agent)
}

/// `command` with `{item}` replaced by the item's ID. IDs are a project name,
/// `#` and a number, so the result needs no quoting.
pub fn wake_command(command: &str, item: &ItemId) -> String {
    command.replace("{item}", &item.to_string())
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
