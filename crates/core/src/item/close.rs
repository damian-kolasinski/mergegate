//! Closing an item, merged or abandoned, and dropping what only an open item
//! needs (design §5).

use super::{Item, ItemError, ItemGit, ItemState, ItemStore};
use crate::diff::DiffStore;
use crate::feedback::{FeedbackStore, Notice};
use crate::store::StoreError;

/// Moves `item` to `state`, a closed one, with `notice` for its agent, then
/// drops its revision pins and the cached diffs no open item still uses.
/// Its file list and line counts stay. A merged item keeps its newest
/// revision's pin: a squash that auto-merged a file the target also changed
/// leaves the item's own blob out of the target's history, and its diff is
/// read from those blobs.
///
/// The close is recorded first, and the cleanup after it can't fail the
/// close: a pin or cache entry left behind only costs disk, while dropping
/// them for an item that stayed open would lose its diff, and reporting a
/// recorded close as failed would hide it from the agent and the UI.
pub(crate) fn close<S: ItemStore + DiffStore + FeedbackStore>(
    store: &mut S,
    git: &impl ItemGit,
    item: &Item,
    state: ItemState,
    notice: Notice,
) -> Result<Item, ItemError> {
    debug_assert!(!state.is_open(), "{state:?} doesn't close an item");
    store.notify(&item.id, state, notice)?;
    let _ = store.prune_diffs(&item.id);
    let _ = git.unpin(item.id.project(), &item.pins());
    if state == ItemState::Merged {
        let _ = git.pin(item.id.project(), &item.pin_ref(), &item.revision.head);
    }
    store
        .item(&item.id)?
        .ok_or_else(|| StoreError::new(format!("{} vanished as it closed", item.id)).into())
}
