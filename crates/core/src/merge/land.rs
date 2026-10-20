//! Landing an item in Merging, for approve and retry alike.

use super::stacked::{follow, open_parent};
use super::{Landing, Merge, MergeBackend, MergeError};
use crate::diff::DiffStore;
use crate::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use crate::highlight::HighlightStore;
use crate::item::{Item, ItemGit, ItemState, ItemStore, close};
use crate::project::{ProjectStore, find};
use crate::store::{Shared, StoreError};
use crate::triage::Guide;

/// Lands `item`, which is in Merging, and tells its agent how that went:
/// merged closes it, a conflict hands it back, anything else holds it.
///
/// An item stacked on one still open waits, untried. One whose parent merged
/// lands its own diff; a conflict leaves it in Merging for its agent to
/// rebase onto the target, and the approval carries if its diff is the same.
/// Once it merges, what's stacked on it follows.
pub(super) fn land<S: ProjectStore + ItemStore + DiffStore + HighlightStore + FeedbackStore>(
    store: &impl Shared<S>,
    git: &impl ItemGit,
    backend: &dyn MergeBackend,
    item: &Item,
) -> Result<Merge, MergeError> {
    let ready = store.with(|store| -> Result<_, MergeError> {
        if let Some(parent) = open_parent(store, item)? {
            return Ok(Err(parent));
        }
        let project = find(store, item.id.project().as_str())?;
        let guide = store.guide(&item.id, item.revision.number)?;
        Ok(Ok((project, message(item, guide.as_deref()))))
    })?;
    let (project, message) = match ready {
        Ok(ready) => ready,
        Err(parent) => {
            return Ok(Merge {
                item: item.clone(),
                landing: Landing::Waiting {
                    parent: parent.name(),
                },
                stacked: Vec::new(),
            });
        }
    };
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
    let recorded = store.with(|store| -> Result<_, MergeError> {
        Ok(match &landing {
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
                // Its own commits are what need rebasing, from the parent
                // revision they're on; the approval stays for when they have.
                let (state, kind) = match &item.parent {
                    Some(parent) => (
                        ItemState::Merging,
                        NoticeKind::ParentUpdated {
                            parent: parent.clone(),
                            merged: true,
                            onto: target_commit.clone(),
                            upstream: item.revision.base.clone(),
                        },
                    ),
                    None => (
                        ItemState::ChangesRequested,
                        NoticeKind::MergeConflict {
                            target_commit: target_commit.clone(),
                            paths: paths.clone(),
                        },
                    ),
                };
                store.notify(&item.id, state, notice(kind))?;
                reread(store, item)?
            }
            Landing::Held { reason } => {
                let kind = NoticeKind::MergeHeld {
                    reason: reason.clone(),
                };
                store.notify(&item.id, ItemState::Merging, notice(kind))?;
                reread(store, item)?
            }
            Landing::Waiting { .. } => reread(store, item)?,
        })
    })?;
    let stacked = match &landing {
        Landing::Merged { commit } => follow(store, git, backend, &project, (&recorded, commit))?,
        _ => Vec::new(),
    };
    Ok(Merge {
        item: recorded,
        landing,
        stacked,
    })
}

/// The squash commit's message: the item's title and name, then the intent
/// its guide states. The guide's file list stays out: the commit shows the
/// files.
fn message(item: &Item, guide: Option<&str>) -> String {
    let subject = format!("{} ({})", item.title, item.name());
    match guide.map(Guide::intent).filter(|intent| !intent.is_empty()) {
        Some(intent) => format!("{subject}\n\n{intent}"),
        None => subject,
    }
}

fn reread(store: &impl ItemStore, item: &Item) -> Result<Item, StoreError> {
    store
        .item(&item.id)?
        .ok_or_else(|| StoreError::new(format!("{} vanished as it merged", item.id)))
}

/// The backend the item was opened with: merging through another would land
/// the work somewhere the item was never meant to go.
pub(super) fn through<'a>(
    item: &Item,
    backends: &[&'a dyn MergeBackend],
) -> Result<&'a dyn MergeBackend, MergeError> {
    backends
        .iter()
        .find(|backend| backend.kind() == item.backend.kind())
        .copied()
        .ok_or_else(|| MergeError::NoBackend {
            id: item.id.clone(),
            backend: item.backend.kind(),
        })
}
