//! Landing a stack bottom first (design §5.1): an approved item waits in
//! Merging for its parent, and lands right after it when its own diff
//! applies onto the target. The hub never writes the item's branch, so an
//! item that can't land that way goes back to its agent to rebase.

use super::land::land;
use super::{MergeBackend, MergeError};
use crate::diff::DiffStore;
use crate::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use crate::highlight::HighlightStore;
use crate::item::{Item, ItemGit, ItemState, ItemStore};
use crate::project::{Project, ProjectStore};
use crate::store::StoreError;

/// The open item `item` is stacked on, which it lands after.
pub(super) fn open_parent(store: &impl ItemStore, item: &Item) -> Result<Option<Item>, StoreError> {
    Ok(match &item.parent {
        Some(id) => store.item(id)?.filter(|parent| parent.state.is_open()),
        None => None,
    })
}

/// After `parent` merged as `commit`: points the item stacked on it at the
/// target, then lands it if it's approved and waiting, or tells its agent to
/// rebase. The parent's branch is tidied away only once its child no longer
/// needs it. Returns every item this moved, up the stack.
pub(super) fn follow<S: ProjectStore + ItemStore + DiffStore + HighlightStore + FeedbackStore>(
    store: &mut S,
    git: &impl ItemGit,
    backend: &dyn MergeBackend,
    project: &Project,
    (parent, commit): (&Item, &str),
) -> Result<Vec<Item>, MergeError> {
    let Some(child) = store.child(&parent.id)? else {
        backend.tidy(project, parent);
        return Ok(Vec::new());
    };
    let rebase = || Notice {
        revision: child.revision.number,
        source: Source::Operator,
        kind: NoticeKind::ParentUpdated {
            parent: parent.id.clone(),
            merged: true,
            onto: commit.to_owned(),
            upstream: child.revision.base.clone(),
        },
    };
    if let Err(e) = backend.restack(project, &child) {
        // The parent's branch stays, so the child's pull request does too;
        // its next submit or a retry points it at the target.
        let notice = match child.state {
            ItemState::Merging => Notice {
                kind: NoticeKind::MergeHeld {
                    reason: format!("{} merged, but {e}", parent.id),
                },
                ..rebase()
            },
            _ => rebase(),
        };
        store.notify(&child.id, child.state, notice)?;
        return Ok(vec![reread(store, &child)?]);
    }
    backend.tidy(project, parent);
    if child.state == ItemState::Merging {
        let merge = land(store, git, backend, &child)?;
        let mut moved = vec![merge.item];
        moved.extend(merge.stacked);
        return Ok(moved);
    }
    store.notify(&child.id, child.state, rebase())?;
    Ok(vec![reread(store, &child)?])
}

fn reread(store: &impl ItemStore, item: &Item) -> Result<Item, StoreError> {
    store
        .item(&item.id)?
        .ok_or_else(|| StoreError::new(format!("{} vanished as its parent merged", item.id)))
}

#[cfg(test)]
#[path = "stacked_tests.rs"]
mod tests;
