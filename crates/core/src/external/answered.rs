//! A review that answers comments its agent never got. The operator can
//! reply to an external comment before voting on it, and the review carries
//! the reply; without the comment, the agent reads a reply to something no
//! command shows it. Replying says the comment matters, so it counts as a
//! vote up, and the comment goes now rather than with a batch that may wait
//! on other votes.

use super::{ExternalError, ExternalStore, Vote, release};
use crate::feedback::Event;
use crate::item::{ItemId, ItemStore};
use crate::review::{Reply, ReviewStore};

/// Votes up the comments of `id` that `replies`, the ones a review just
/// sent, answer before a batch carried them, and sends every held comment
/// voted up. None when they answer none.
pub fn send_answered<S: ItemStore + ReviewStore + ExternalStore>(
    store: &mut S,
    id: &ItemId,
    replies: &[Reply],
    now: u64,
) -> Result<Option<Event>, ExternalError> {
    let mut answered = false;
    for reply in replies {
        let Some(external) = store.comment(reply.comment)?.and_then(|c| c.external) else {
            continue;
        };
        // A summary never goes to the agent, as `vote` refuses it.
        if external.summary || !(external.pending || external.dismissed()) {
            continue;
        }
        if external.vote != Some(Vote::Up) {
            store.set_vote(reply.comment, Vote::Up, now)?;
        }
        answered = true;
    }
    if !answered {
        return Ok(None);
    }
    release(store, id)
}

#[cfg(test)]
#[path = "answered_tests.rs"]
mod tests;
