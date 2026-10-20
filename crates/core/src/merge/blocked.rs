//! Why an open item's merge didn't land, while that still stands, for the
//! inbox and the review screen.

use crate::feedback::{EventKind, FeedbackStore, NoticeKind};
use crate::item::{Item, ItemId, ItemState, ItemStore};
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// Held in Merging until the operator retries or cancels.
    Held { reason: String },
    /// Back with its agent to rebase.
    Conflict {
        target_commit: String,
        paths: Vec<String>,
    },
    /// Approved, and lands right after `parent`, which it's stacked on.
    Waiting { parent: ItemId },
    /// Approved, but its own diff didn't apply once `parent` merged; it lands
    /// when its agent has rebased, on the same approval if the diff is
    /// unchanged.
    Behind { parent: ItemId },
}

/// The newest revision's last merge event says it, as long as the item is
/// still where that event put it: a new revision, a cancel or another
/// approval moves on from it. Reviews in between don't.
///
/// An item seen in Merging with no hold logged is a merge still landing
/// while `landing` says one may be, as it may when read after the store;
/// otherwise a stopped hub or a failed write cut it short, and only the
/// operator moves it on.
pub fn blocked(
    store: &(impl FeedbackStore + ItemStore),
    item: &Item,
    landing: bool,
) -> Result<Option<Blocked>, StoreError> {
    if item.state == ItemState::Merging
        && let Some(parent) = &item.parent
        && store.item(parent)?.is_some_and(|p| p.state.is_open())
    {
        return Ok(Some(Blocked::Waiting {
            parent: parent.clone(),
        }));
    }
    let last = store
        .events(&item.id, 0)?
        .into_iter()
        .filter_map(|event| match event.kind {
            EventKind::Notice(notice) if notice.revision == item.revision.number => {
                Some(notice.kind)
            }
            _ => None,
        })
        .rfind(|kind| !matches!(kind, NoticeKind::Abandoned { .. }));
    Ok(match (last, item.state) {
        (Some(NoticeKind::MergeHeld { reason }), ItemState::Merging) => {
            Some(Blocked::Held { reason })
        }
        (
            Some(NoticeKind::ParentUpdated {
                parent,
                merged: true,
                ..
            }),
            ItemState::Merging,
        ) => Some(Blocked::Behind { parent }),
        (_, ItemState::Merging) if landing => None,
        (_, ItemState::Merging) => Some(Blocked::Held {
            reason: "the merge stopped before it finished".into(),
        }),
        (
            Some(NoticeKind::MergeConflict {
                target_commit,
                paths,
            }),
            ItemState::ChangesRequested,
        ) => Some(Blocked::Conflict {
            target_commit,
            paths,
        }),
        _ => None,
    })
}

#[cfg(test)]
#[path = "blocked_tests.rs"]
mod tests;
