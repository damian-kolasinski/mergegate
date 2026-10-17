//! What an item's agent waits on the operator for in its threads, which
//! puts the item under Needs you whatever its state.

use super::{Author, Comment, Reply, ReplyStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Waiting {
    /// Threads whose agent asked last.
    pub questions: u32,
    /// GitHub comments whose agent rejected them last, until the operator
    /// agrees or overrides. A rejection of the operator's own comment asks
    /// nothing of them, so it doesn't count.
    pub rejections: u32,
    /// GitHub comments held from the agent until the operator votes on them.
    pub votes: u32,
}

impl Waiting {
    pub fn any(&self) -> bool {
        self.questions > 0 || self.rejections > 0 || self.votes > 0
    }
}

/// Only sent replies count: a draft answer hasn't reached the agent yet.
pub fn waiting(comments: &[Comment], replies: &[Reply]) -> Waiting {
    let mut waiting = Waiting::default();
    for comment in comments {
        if comment.external.as_ref().is_some_and(|e| e.awaiting_vote()) {
            waiting.votes += 1;
        }
        // Replies are oldest first, so the last one says who spoke last.
        let Some(last) = replies
            .iter()
            .rfind(|r| r.comment == comment.id && !r.is_draft())
        else {
            continue;
        };
        if last.author != Author::Agent {
            continue;
        }
        match last.status {
            Some(ReplyStatus::Question) => waiting.questions += 1,
            Some(ReplyStatus::Rejected) => {
                let agreed = comment
                    .external
                    .as_ref()
                    .map(|external| external.agreed_reply == Some(last.id));
                if agreed == Some(false) {
                    waiting.rejections += 1;
                }
            }
            _ => {}
        }
    }
    waiting
}

#[cfg(test)]
#[path = "waiting_tests.rs"]
mod tests;
