//! The operator's vote on a comment from GitHub before it reaches the agent.
//! Reviewers, bots above all, comment on everything, and an agent that acts
//! on every comment spends its time on noise, so each waits for the
//! operator: up sends it with the next batch, down keeps it from the agent
//! for good.

use super::{ExternalError, ExternalStore, Vote};
use crate::item::ItemStore;
use crate::review::{Comment, CommentId, ReviewError, ReviewStore, reviewable};

/// Records the operator's `vote` on external `comment`. They can change it
/// until a batch carries the comment, and vote up one they voted down.
pub fn vote<S: ItemStore + ReviewStore + ExternalStore>(
    store: &mut S,
    comment: CommentId,
    vote: Vote,
    now: u64,
) -> Result<Comment, ExternalError> {
    let found = store
        .comment(comment)?
        .ok_or(ExternalError::NoComment(comment))?;
    let external = found
        .external
        .as_ref()
        .ok_or(ExternalError::NoComment(comment))?;
    if external.summary {
        return Err(ExternalError::Summary(comment));
    }
    if !external.pending && !external.dismissed() {
        return Err(ExternalError::AlreadySent(comment));
    }
    let item = store
        .item(&found.item)?
        .ok_or_else(|| ReviewError::NoItem(found.item.clone()))?;
    reviewable(&item)?;
    store.set_vote(comment, vote, now)?;
    store
        .comment(comment)?
        .ok_or(ExternalError::NoComment(comment))
}

#[cfg(test)]
#[path = "vote_tests.rs"]
mod tests;
