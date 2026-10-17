//! The comments an item's agent still owes something on, which `mg resume`
//! hands a fresh session so it needn't replay the event log.

use super::{Author, Comment, Conversation, Reply, ReplyStatus};

/// A sent comment and its sent replies, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    pub comment: Comment,
    pub replies: Vec<Reply>,
}

/// The sent comments that aren't settled, oldest first; an external one is
/// sent once its batch went, so a session started meanwhile isn't handed it
/// ahead of the batch, which would wake it again, and one the operator voted
/// down never is. One is settled when
/// its agent's latest reply rejects it, or addresses it in a revision
/// submitted since, and the operator hasn't replied after that. Anything else
/// needs the agent: an answer, a revision with the change, or the reply
/// to an operator who wrote back.
pub fn open_threads(conversation: &Conversation) -> Vec<Thread> {
    conversation
        .comments
        .iter()
        .filter(|c| {
            !c.is_draft()
                && !c
                    .external
                    .as_ref()
                    .is_some_and(|e| e.pending || e.dismissed())
        })
        .map(|comment| Thread {
            comment: comment.clone(),
            replies: conversation
                .replies
                .iter()
                .filter(|r| r.comment == comment.id && !r.is_draft())
                .cloned()
                .collect(),
        })
        .filter(|thread| !settled(thread, conversation.newest))
        .collect()
}

fn settled(thread: &Thread, newest: u32) -> bool {
    // Replies are oldest first, so the last one says who spoke last.
    let Some(last) = thread.replies.last() else {
        return false;
    };
    match (last.author, last.status) {
        (Author::Agent, Some(ReplyStatus::Rejected)) => true,
        (Author::Agent, Some(ReplyStatus::Addressed)) => last.addressed_in(newest).is_some(),
        _ => false,
    }
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
