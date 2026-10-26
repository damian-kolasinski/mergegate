//! What a fresh agent session needs to pick up an item whose session ended:
//! the item, where its agent worked, the last review, the comments still
//! open and the notice still asking for a revision. It reads no delivered
//! cursor and acknowledges nothing, so the old session's unread events still
//! reach `mg wait`.

use crate::agent::AgentStore;
use crate::feedback::{Event, EventKind, FeedbackStore, NoticeKind};
use crate::item::{Item, ItemId, ItemState, ItemStore};
use crate::review::{Review, ReviewStore, Thread, open_threads, read_conversation};
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resume {
    pub item: Item,
    /// Where the newest submit came from; none from an `mg` that didn't say.
    pub worktree: Option<String>,
    /// The wake command an earlier session left, which would start that
    /// session rather than the new one.
    pub wake: Option<String>,
    /// The newest review the operator sent.
    pub review: Option<Review>,
    pub open: Vec<Thread>,
    /// The notice that asks for another revision, while it still stands. It
    /// is the only record of a conflict or a moved parent, and a wait whose
    /// output was lost delivered it all the same.
    pub pending: Option<Event>,
}

/// `None` when there's no such item.
pub fn resume<S: ItemStore + ReviewStore + FeedbackStore + AgentStore>(
    store: &S,
    id: &ItemId,
) -> Result<Option<Resume>, StoreError> {
    let Some(item) = store.item(id)? else {
        return Ok(None);
    };
    let events = store.events(id, 0)?;
    let review = events.iter().rev().find_map(|event| match &event.kind {
        EventKind::ReviewSubmitted(review) => Some(review.clone()),
        EventKind::Notice(_) | EventKind::ExternalComments(_) | EventKind::Override(_) => None,
    });
    let pending = pending(events, &item);
    let conversation = read_conversation(store, id, item.revision.number)?;
    let agent = store.agent(id)?;
    Ok(Some(Resume {
        worktree: agent.worktree,
        wake: agent.wake,
        review,
        open: open_threads(&conversation),
        pending,
        item,
    }))
}

/// The newest revision's last notice, as [`crate::merge::blocked`] reads it,
/// when it asks for a revision and the item is still where it put the item.
/// An earlier one was replaced by it, as a parent's second revision replaces
/// the rebase its first asked for, and one on an older revision was answered
/// by a newer revision. An approved item waiting on its parent lands without
/// a rebase until the parent merges without it.
fn pending(events: Vec<Event>, item: &Item) -> Option<Event> {
    let last = events.into_iter().rfind(|event| match &event.kind {
        EventKind::Notice(notice) => {
            notice.revision == item.revision.number
                && !matches!(notice.kind, NoticeKind::Abandoned { .. })
        }
        _ => false,
    })?;
    let EventKind::Notice(notice) = &last.kind else {
        return None;
    };
    let with_agent = matches!(
        item.state,
        ItemState::NeedsReview | ItemState::ChangesRequested
    );
    let asks = match notice.kind {
        NoticeKind::MergeConflict { .. } => item.state == ItemState::ChangesRequested,
        NoticeKind::ParentUpdated { merged: true, .. } => item.state.is_open(),
        NoticeKind::ParentUpdated { merged: false, .. } | NoticeKind::ParentAbandoned { .. } => {
            with_agent
        }
        NoticeKind::Abandoned { .. }
        | NoticeKind::MergeHeld { .. }
        | NoticeKind::MergeCancelled
        | NoticeKind::Merged { .. } => false,
    };
    asks.then_some(last)
}

#[cfg(test)]
#[path = "resume_tests.rs"]
mod tests;
