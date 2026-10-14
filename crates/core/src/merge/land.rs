//! Landing an item in Merging, for approve and retry alike.

use super::{Landing, Merge, MergeBackend, MergeError};
use crate::diff::DiffStore;
use crate::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use crate::item::{Item, ItemGit, ItemState, ItemStore, close};
use crate::project::{ProjectStore, find};
use crate::store::StoreError;

/// Lands `item`, which is in Merging, and tells its agent how that went:
/// merged closes it, a conflict hands it back, anything else holds it.
pub(super) fn land<S: ProjectStore + ItemStore + DiffStore + FeedbackStore>(
    store: &mut S,
    git: &impl ItemGit,
    backend: &impl MergeBackend,
    item: &Item,
) -> Result<Merge, MergeError> {
    let project = find(store, item.id.project().as_str())?;
    let message = format!("{} ({})", item.title, item.id);
    // A failure to even try still leaves the item waiting on the operator,
    // who sees why and can retry.
    let landing = backend
        .land(&project, item, &message)
        .unwrap_or_else(|e| Landing::Held {
            reason: e.to_string(),
        });
    let notice = |kind| Notice {
        revision: item.revision.number,
        source: Source::Operator,
        kind,
    };
    let item = match &landing {
        Landing::Merged { commit } => {
            let kind = NoticeKind::Merged {
                commit: commit.clone(),
            };
            close(store, git, item, ItemState::Merged, notice(kind))?
        }
        Landing::Conflict {
            target_commit,
            paths,
        } => {
            let kind = NoticeKind::MergeConflict {
                target_commit: target_commit.clone(),
                paths: paths.clone(),
            };
            store.notify(&item.id, ItemState::ChangesRequested, notice(kind))?;
            reread(store, item)?
        }
        Landing::Held { reason } => {
            let kind = NoticeKind::MergeHeld {
                reason: reason.clone(),
            };
            store.notify(&item.id, ItemState::Merging, notice(kind))?;
            reread(store, item)?
        }
    };
    Ok(Merge { item, landing })
}

fn reread(store: &impl ItemStore, item: &Item) -> Result<Item, StoreError> {
    store
        .item(&item.id)?
        .ok_or_else(|| StoreError::new(format!("{} vanished as it merged", item.id)))
}

/// The item's backend must be the one given: merging through another would
/// land the work somewhere the item was never meant to go.
pub(super) fn through(item: &Item, backend: &impl MergeBackend) -> Result<(), MergeError> {
    if item.backend != backend.kind() {
        return Err(MergeError::NoBackend {
            id: item.id.clone(),
            backend: item.backend,
        });
    }
    Ok(())
}
