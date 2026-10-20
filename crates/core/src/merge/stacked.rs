//! Landing a stack bottom first: an approved item waits in
//! Merging for its parent, and lands right after it when its own diff
//! applies onto the target. The hub never writes the item's branch, so an
//! item that can't land that way goes back to its agent to rebase.

use super::land::{land, through};
use super::{MergeBackend, MergeError};
use crate::diff::DiffStore;
use crate::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use crate::highlight::HighlightStore;
use crate::item::{Item, ItemGit, ItemId, ItemState, ItemStore};
use crate::project::{Project, ProjectStore, find};
use crate::store::{Shared, StoreError};

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
    store: &impl Shared<S>,
    git: &impl ItemGit,
    backend: &dyn MergeBackend,
    project: &Project,
    (parent, commit): (&Item, &str),
) -> Result<Vec<Item>, MergeError> {
    let Some(child) = store.with(|store| store.child(&parent.id))? else {
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
                    reason: format!("{} merged, but {e}", parent.name()),
                },
                ..rebase()
            },
            _ => rebase(),
        };
        return told(store, &child, notice);
    }
    backend.tidy(project, parent);
    if child.state == ItemState::Merging {
        let merge = land(store, git, backend, &child)?;
        let mut moved = vec![merge.item];
        moved.extend(merge.stacked);
        return Ok(moved);
    }
    told(store, &child, rebase())
}

/// `child` once its agent has `notice`, in the state it's in now: the
/// operator may have reviewed it, or the conflict watch handed it back,
/// while its parent's merge called the backend.
fn told<S: ItemStore + FeedbackStore>(
    store: &impl Shared<S>,
    child: &Item,
    notice: Notice,
) -> Result<Vec<Item>, MergeError> {
    store.with(|store| {
        let child = reread(store, child)?;
        store.notify(&child.id, child.state, notice)?;
        Ok(vec![reread(store, &child)?])
    })
}

/// After `id` merged as `commit` other than through the hub, as on GitHub:
/// what's stacked on it follows as it would the hub's own merge, and its
/// branch goes once nothing needs it. Returns every item this moved.
pub fn merged_elsewhere<
    S: ProjectStore + ItemStore + DiffStore + HighlightStore + FeedbackStore,
>(
    store: &impl Shared<S>,
    git: &impl ItemGit,
    backends: &[&dyn MergeBackend],
    id: &ItemId,
    commit: &str,
) -> Result<Vec<Item>, MergeError> {
    let (item, project) = store.with(|store| -> Result<_, MergeError> {
        let item = store
            .item(id)?
            .ok_or_else(|| MergeError::NoItem(id.clone()))?;
        Ok((item, find(store, id.project().as_str())?))
    })?;
    let backend = through(&item, backends)?;
    follow(store, git, backend, &project, (&item, commit))
}

/// Tidies away the branch of `item`'s merged parent, which [`follow`] left
/// when it couldn't move `item`'s review off it, unless a new item has
/// taken its name since.
pub(super) fn tidy_parent<S: ProjectStore + ItemStore>(
    store: &impl Shared<S>,
    backend: &dyn MergeBackend,
    item: &Item,
) -> Result<(), MergeError> {
    let tidy = store.with(|store| -> Result<_, MergeError> {
        let parent = match &item.parent {
            Some(parent) => store.item(parent)?,
            None => None,
        };
        let Some(parent) = parent.filter(|p| p.state == ItemState::Merged) else {
            return Ok(None);
        };
        if store
            .open_item(item.id.project(), &parent.branch)?
            .is_some()
        {
            return Ok(None);
        }
        Ok(Some((find(store, item.id.project().as_str())?, parent)))
    })?;
    if let Some((project, parent)) = tidy {
        backend.tidy(&project, &parent);
    }
    Ok(())
}

fn reread(store: &impl ItemStore, item: &Item) -> Result<Item, StoreError> {
    store
        .item(&item.id)?
        .ok_or_else(|| StoreError::new(format!("{} vanished as its parent merged", item.id)))
}

#[cfg(test)]
#[path = "stacked_tests.rs"]
mod tests;
