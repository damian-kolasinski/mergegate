//! What a fresh agent session needs to pick up an item whose session ended:
//! the item, where its agent worked, the last review and the comments
//! still open. It reads no delivered cursor and acknowledges nothing, so the
//! old session's unread events still reach `mg wait`.

use crate::agent::AgentStore;
use crate::feedback::{EventKind, FeedbackStore};
use crate::item::{Item, ItemId, ItemStore};
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
}

/// `None` when there's no such item.
pub fn resume<S: ItemStore + ReviewStore + FeedbackStore + AgentStore>(
    store: &S,
    id: &ItemId,
) -> Result<Option<Resume>, StoreError> {
    let Some(item) = store.item(id)? else {
        return Ok(None);
    };
    let review = store
        .events(id, 0)?
        .into_iter()
        .rev()
        .find_map(|event| match event.kind {
            EventKind::ReviewSubmitted(review) => Some(review),
            EventKind::Notice(_) | EventKind::ExternalComments(_) | EventKind::Override(_) => None,
        });
    let conversation = read_conversation(store, id, item.revision.number)?;
    let agent = store.agent(id)?;
    Ok(Some(Resume {
        worktree: agent.worktree,
        wake: agent.wake,
        review,
        open: open_threads(&conversation),
        item,
    }))
}

#[cfg(test)]
#[path = "resume_tests.rs"]
mod tests;
