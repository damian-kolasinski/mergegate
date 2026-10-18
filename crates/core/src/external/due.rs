//! When held comments go to the agent. The hub waits so the agent doesn't:
//! reviewers post one comment at a time, bots take minutes, and an agent
//! woken on every comment would spend a session on each. Each comment also
//! waits for the operator's vote, which only they can give.

use super::{ExternalError, ExternalStore, Held, same_login};
use crate::feedback::Event;
use crate::item::{ItemId, ItemState, ItemStore};
use crate::project::ProjectStore;
use crate::review::ReviewError;

/// In milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// How long nothing may change before a batch goes.
    pub quiet: u64,
    /// How long a batch waits at most, for expected bots and in all.
    pub cap: u64,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            quiet: 60_000,
            cap: 20 * 60_000,
        }
    }
}

/// Whether the held comments go now. Never while one awaits the operator's
/// vote: the batch waits for the operator as it does for the bots, but with
/// no cap, since only they can say which comments are worth sending. Then
/// once they have been quiet for a while and every bot in
/// `expected` has spoken since the newest revision was pushed, or enough time
/// passed since the push that one never will. They go regardless once the
/// oldest has waited the cap, so a busy thread can't hold them forever.
pub fn due(held: &Held, expected: &[String], now: u64, timing: Timing) -> bool {
    if held.awaiting {
        return false;
    }
    if now >= held.first.saturating_add(timing.cap) {
        return true;
    }
    if now < held.last.saturating_add(timing.quiet) {
        return false;
    }
    let heard = |bot: &String| held.heard.iter().any(|login| same_login(login, bot));
    expected.iter().all(heard) || now >= held.pushed.saturating_add(timing.cap)
}

/// Sends `id`'s held comments to its agent when they're [`due`].
pub fn release_due<S: ItemStore + ProjectStore + ExternalStore>(
    store: &mut S,
    id: &ItemId,
    now: u64,
    timing: Timing,
) -> Result<Option<Event>, ExternalError> {
    let Some(held) = store.held(id)? else {
        return Ok(None);
    };
    let expected = store
        .project(id.project())?
        .map(|project| project.bots)
        .unwrap_or_default();
    if !due(&held, &expected, now, timing) {
        return Ok(None);
    }
    release(store, id)
}

/// Sends `id`'s held comments to its agent now, while the agent can answer
/// them: a merging item's wait until the merge settles.
pub fn release<S: ItemStore + ExternalStore>(
    store: &mut S,
    id: &ItemId,
) -> Result<Option<Event>, ExternalError> {
    let item = store
        .item(id)?
        .ok_or_else(|| ReviewError::NoItem(id.clone()))?;
    match item.state {
        ItemState::NeedsReview | ItemState::ChangesRequested => {
            Ok(store.release(id, item.revision.number)?)
        }
        ItemState::Merging | ItemState::Merged | ItemState::Abandoned => Ok(None),
    }
}

#[cfg(test)]
#[path = "due_tests.rs"]
mod tests;
