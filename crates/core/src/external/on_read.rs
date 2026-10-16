//! A read that hands the agent events takes the held comments along: the
//! agent is awake anyway, and their batch would wake it again soon after.

use super::{ExternalError, ExternalStore, release};
use crate::feedback::{Event, FeedbackStore};
use crate::item::{ItemId, ItemStore};

/// Releases `id`'s held comments when a read of what's new is about to hand
/// `handed` to the agent; whether it did. A read is of what's new unless its
/// `since` looks back before the delivered cursor: `mg wait --since` the
/// cursor the last read printed reads on from it. A look back, and a read
/// that hands nothing over, change nothing, so a wait that keeps looking
/// without returning never cuts a batch short.
pub fn release_on_read<S: ItemStore + ExternalStore + FeedbackStore>(
    store: &mut S,
    id: &ItemId,
    since: Option<u64>,
    handed: &[Event],
) -> Result<bool, ExternalError> {
    if handed.is_empty() {
        return Ok(false);
    }
    if let Some(since) = since
        && since < store.delivered(id)?
    {
        return Ok(false);
    }
    Ok(release(store, id)?.is_some())
}

#[cfg(test)]
#[path = "on_read_tests.rs"]
mod tests;
