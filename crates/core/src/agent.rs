//! What an item's agent leaves the hub when it submits: the worktree it works
//! in, which `mg resume` names, and the command that wakes it when the item
//! has events.

use crate::feedback::{FeedbackStore, Source};
use crate::item::{Item, ItemId, ItemState};
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Agent {
    /// The absolute path of the worktree the newest submit came from.
    pub worktree: Option<String>,
    /// A shell command run when the item has events from the operator its
    /// agent hasn't read.
    pub wake: Option<String>,
    /// The tailnet device the newest submit came from, whose `mg wakes`
    /// runner runs the wake; none for the hub's machine, where the hub does.
    pub device: Option<String>,
}

/// An item whose agent is to be woken: it has a wake command and an event
/// from the operator newer than both the one its agent acknowledged and the
/// one that woke it last. The agent's own events, like its `mg abandon`,
/// never wake it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueWake {
    pub item: ItemId,
    /// What its agent knows it by, as in [`crate::item::Item::name`].
    pub name: ItemId,
    pub command: String,
    pub worktree: Option<String>,
    /// As in [`Agent::device`].
    pub device: Option<String>,
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

/// Records what a submit from `device` carried. A submit without a worktree
/// or wake keeps the item's; an empty wake removes it.
pub(crate) fn record<S: AgentStore>(
    store: &mut S,
    id: &ItemId,
    worktree: Option<String>,
    wake: Option<String>,
    device: Option<String>,
) -> Result<(), StoreError> {
    let mut agent = store.agent(id)?;
    if worktree.is_none() && wake.is_none() && agent.device == device {
        return Ok(());
    }
    // The item moved to another machine, where the wake left on the old
    // one would start a session in a worktree its agent isn't in.
    if agent.device != device {
        agent.device = device;
        agent.wake = None;
    }
    if worktree.is_some() {
        agent.worktree = worktree;
    }
    if let Some(wake) = wake {
        let wake = wake.trim();
        agent.wake = (!wake.is_empty()).then(|| wake.to_owned());
    }
    store.set_agent(id, &agent)
}

/// Whether what the operator sends an open item reaches an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// Something reads the item's feedback now, or did moments ago: an
    /// `mg wait` blocked on it, or one between two timed-out waits.
    Waiting,
    /// Back with its agent, who has read everything sent to it and is
    /// presumably working on it.
    Working,
    /// Nothing reads it, but the hub runs the item's wake command for what
    /// the operator sends.
    Wakes,
    /// Nothing reads it and nothing wakes one: what the operator sends waits
    /// until a session picks the item up.
    Away,
}

/// `item`'s [`Presence`], none once it's closed. Only the hub knows whether
/// a read of its feedback is `reading`, and whether its wake command `wakes`,
/// which takes a worktree that is still there.
pub fn presence<S: FeedbackStore>(
    store: &S,
    item: &Item,
    reading: bool,
    wakes: bool,
) -> Result<Option<Presence>, StoreError> {
    if !item.state.is_open() {
        return Ok(None);
    }
    if reading {
        return Ok(Some(Presence::Waiting));
    }
    // Under review, or merging, the agent should be waiting for what comes
    // next. Back with it, an agent that read what was sent is off doing it.
    if item.state == ItemState::ChangesRequested {
        let delivered = store.delivered(&item.id)?;
        let unread = store
            .events(&item.id, delivered)?
            .iter()
            .any(|e| e.source() != Source::Agent);
        if !unread {
            return Ok(Some(Presence::Working));
        }
    }
    Ok(Some(if wakes {
        Presence::Wakes
    } else {
        Presence::Away
    }))
}

/// Variables a coding agent's harness sets for its own session. Commands the
/// hub runs drop them: a hub started from an agent's shell inherits them, and
/// a harness that sees them takes itself for a session nested in that one,
/// which some refuse to start. Config locations like `CODEX_HOME` stay.
pub const SESSION_MARKERS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_PID",
    "CODEX_THREAD_ID",
    "CODEX_SESSION_ID",
    "CODEX_APP_TOOLS_PIPE_PATH",
    "CODEX_INTERNAL_ORIGINATOR_OVERRIDE",
];

/// `command` with `{item}` replaced by the item's name. Names are a project
/// name, `#` and a number, so the result needs no quoting.
pub fn wake_command(command: &str, item: &ItemId) -> String {
    command.replace("{item}", &item.to_string())
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
