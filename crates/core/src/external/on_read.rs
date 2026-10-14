//! A read that hands the agent events takes the held comments along: the
//! agent is awake anyway, and their batch would wake it again soon after.

use super::{ExternalError, ExternalStore, release};
use crate::feedback::Event;
use crate::item::{ItemId, ItemStore};

/// Releases `id`'s held comments when a read of what's new (`since` unset)
/// is about to hand `handed` to the agent; whether it did. A look back from
/// a given cursor, and a read that hands nothing over, change nothing, so a
/// wait that keeps looking without returning never cuts a batch short.
pub fn release_on_read<S: ItemStore + ExternalStore>(
    store: &mut S,
    id: &ItemId,
    since: Option<u64>,
    handed: &[Event],
) -> Result<bool, ExternalError> {
    if since.is_some() || handed.is_empty() {
        return Ok(false);
    }
    Ok(release(store, id)?.is_some())
}
