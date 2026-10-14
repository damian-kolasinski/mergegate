//! Why an open item's merge didn't land, while that still stands, for the
//! inbox and the review screen.

use crate::feedback::{EventKind, FeedbackStore, NoticeKind};
use crate::item::{Item, ItemState};
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
}

/// The newest revision's last merge event says it, as long as the item is
/// still where that event put it: a new revision, a cancel or another
/// approval moves on from it. Reviews in between don't.
///
/// An item in Merging is always held: approve and retry hold the store until
/// they finish, so one seen in Merging with no hold logged is a merge that a
/// stopped hub or a failed write cut short, and only the operator moves it on.
pub fn blocked(store: &impl FeedbackStore, item: &Item) -> Result<Option<Blocked>, StoreError> {
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
