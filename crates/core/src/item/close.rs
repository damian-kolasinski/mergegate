//! Closing an item, merged or abandoned, and dropping what only an open item
//! needs (design §5).

use super::{Item, ItemError, ItemGit, ItemState, ItemStore};
use crate::diff::DiffStore;
use crate::feedback::{FeedbackStore, Notice};
use crate::highlight::HighlightStore;
use crate::store::StoreError;

/// Moves `item` to `state`, a closed one, with `notice` for its agent, then
/// drops its revision pins, its branch and the cached diffs and spans no
/// open item still uses.
/// The branch goes so it doesn't keep names that nest with it (`a/b` beside
/// `a`) from being pushed, as git refuses those; nothing reads it once the
/// item is closed. It stays if it moved past the newest revision, which is
/// someone's work the hub never saw.
/// Its file list and line counts stay. A merged item keeps its newest
/// revision's pin: a squash that auto-merged a file the target also changed
/// leaves the item's own blob out of the target's history, and its diff is
/// read from those blobs.
///
/// The close is recorded first, and the cleanup after it can't fail the
/// close: a pin or cache entry left behind only costs disk, while dropping
/// them for an item that stayed open would lose its diff, and reporting a
/// recorded close as failed would hide it from the agent and the UI.
pub(crate) fn close<S: ItemStore + DiffStore + HighlightStore + FeedbackStore>(
    store: &mut S,
    git: &impl ItemGit,
    item: &Item,
    state: ItemState,
    notice: Notice,
) -> Result<Item, ItemError> {
    debug_assert!(!state.is_open(), "{state:?} doesn't close an item");
    store.notify(&item.id, state, notice)?;
    let _ = store.prune_diffs(&item.id);
    let _ = store.prune_highlights(&item.id);
    let _ = git.unpin(item.id.project(), &item.pins());
    let _ = git.drop_branch(item.id.project(), &item.branch, &item.revision.head);
    if state == ItemState::Merged {
        let _ = git.pin(item.id.project(), &item.pin_ref(), &item.revision.head);
    }
    store
        .item(&item.id)?
        .ok_or_else(|| StoreError::new(format!("{} vanished as it closed", item.id)).into())
}
