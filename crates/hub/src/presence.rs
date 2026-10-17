//! Whether an agent is there to read what the operator sends an item, for
//! the review screen, which warns when none is. A read of the item's
//! feedback, an `mg wait` blocking on it or an `mg feedback`, is an agent
//! there, and stays one for [`GRACE`] after it ends: an agent looping
//! `mg wait --timeout N` reads nothing between two waits.
//!
//! Kept in memory, so a restarted hub sees no agent until each reconnects.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use api::agent::AgentPresence;
use api::event::UiEvent;
use mergegate_core::agent::{self, AgentStore, Presence};
use mergegate_core::feedback::FeedbackStore;
use mergegate_core::item::{Item, ItemId};
use mergegate_core::store::StoreError;
use tokio::time::Instant;

use crate::server::AppState;

/// How long an agent stays there after its last read ends.
const GRACE: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(crate) struct Readers {
    inner: Mutex<HashMap<ItemId, Reads>>,
}

#[derive(Default)]
struct Reads {
    open: usize,
    /// When the last read ended, while none is open.
    ended: Option<Instant>,
}

impl Reads {
    fn there(&self, now: Instant) -> bool {
        self.open > 0
            || self
                .ended
                .is_some_and(|ended| now.saturating_duration_since(ended) < GRACE)
    }
}

impl Readers {
    fn there(&self, item: &ItemId, now: Instant) -> bool {
        self.lock().get(item).is_some_and(|reads| reads.there(now))
    }

    /// A read began; whether it brought an agent back.
    fn begin(&self, item: &ItemId, now: Instant) -> bool {
        let mut inner = self.lock();
        let reads = inner.entry(item.clone()).or_default();
        let back = !reads.there(now);
        reads.open += 1;
        reads.ended = None;
        back
    }

    fn end(&self, item: &ItemId, now: Instant) {
        let mut inner = self.lock();
        if let Some(reads) = inner.get_mut(item) {
            reads.open = reads.open.saturating_sub(1);
            if reads.open == 0 {
                reads.ended = Some(now);
            }
        }
    }

    /// Forgets the item's agent once its grace ran out with no read since;
    /// whether it did. A read that ended later expires on its own timer.
    fn expire(&self, item: &ItemId, now: Instant) -> bool {
        let mut inner = self.lock();
        let gone = inner.get(item).is_some_and(|reads| !reads.there(now));
        if gone {
            inner.remove(item);
        }
        gone
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<ItemId, Reads>> {
        // Each update is one entry's, so a panic leaves none half-written.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Held for the length of a read of an item's feedback.
pub(crate) struct Reading {
    state: Arc<AppState>,
    item: ItemId,
}

pub(crate) fn reading(state: &Arc<AppState>, item: &ItemId) -> Reading {
    if state.presence.begin(item, Instant::now()) {
        changed(state, item);
    }
    Reading {
        state: Arc::clone(state),
        item: item.clone(),
    }
}

impl Drop for Reading {
    fn drop(&mut self) {
        self.state.presence.end(&self.item, Instant::now());
        // Off the runtime the hub is stopping, and no UI is left to tell.
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let state = Arc::clone(&self.state);
        let item = self.item.clone();
        runtime.spawn(async move {
            tokio::time::sleep(GRACE).await;
            if state.presence.expire(&item, Instant::now()) {
                changed(&state, &item);
            }
        });
    }
}

/// The review screen refetches the item, and with it the presence.
fn changed(state: &AppState, item: &ItemId) {
    state.events.send(UiEvent::ItemChanged {
        item: item.to_string(),
    });
}

/// `item`'s presence, none once it's closed.
pub(crate) fn of<S: FeedbackStore + AgentStore>(
    store: &S,
    readers: &Readers,
    item: &Item,
) -> Result<Option<AgentPresence>, StoreError> {
    let reading = readers.there(&item.id, Instant::now());
    // As the wake runner sees it: a command runs only in a worktree that is
    // still there.
    let left = store.agent(&item.id)?;
    let wakes = left.wake.is_some()
        && left
            .worktree
            .as_deref()
            .is_some_and(|worktree| Path::new(worktree).is_dir());
    Ok(agent::presence(store, item, reading, wakes)?.map(to_api))
}

fn to_api(presence: Presence) -> AgentPresence {
    match presence {
        Presence::Waiting => AgentPresence::Waiting,
        Presence::Working => AgentPresence::Working,
        Presence::Wakes => AgentPresence::Wakes,
        Presence::Away => AgentPresence::Away,
    }
}

#[cfg(test)]
#[path = "presence_tests.rs"]
mod tests;
