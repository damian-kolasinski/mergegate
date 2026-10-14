//! The operator's say on the agent's rejection of an external comment. The
//! reviewer on GitHub can't see the agent's reply, so the operator decides:
//! agree, which only marks the thread, or override, which sends the comment
//! back to the agent. Nothing goes to GitHub.

use super::{ExternalError, ExternalStore};
use crate::feedback::Event;
use crate::item::{Item, ItemStore};
use crate::review::{
    Author, Comment, CommentId, NewReply, Reply, ReplyStatus, ReviewError, ReviewStore, reviewable,
};

/// Marks the agent's rejection of `comment` as one the operator agrees with.
pub fn agree<S: ItemStore + ReviewStore + ExternalStore>(
    store: &mut S,
    comment: CommentId,
) -> Result<Comment, ExternalError> {
    let (mut found, _, rejection) = rejected(store, comment)?;
    store.set_agreed(comment, rejection.id)?;
    if let Some(external) = &mut found.external {
        external.agreed_reply = Some(rejection.id);
    }
    Ok(found)
}

/// Tells the agent the operator wants the change it rejected, with `note`
/// saying why when they wrote one, at once rather than with the next review.
/// The thread is open again until the agent answers.
pub fn override_rejection<S: ItemStore + ReviewStore + ExternalStore>(
    store: &mut S,
    comment: CommentId,
    note: String,
) -> Result<(Reply, Event), ExternalError> {
    let (_, item, _) = rejected(store, comment)?;
    let reply = NewReply {
        comment,
        item: item.id,
        author: Author::Operator,
        status: None,
        body: note.trim().to_owned(),
        revision: item.revision.number,
    };
    Ok(store.insert_override(reply)?)
}

/// The external comment, its item and the agent's rejection, when that is
/// the thread's latest word and the item still takes feedback.
fn rejected<S: ItemStore + ReviewStore>(
    store: &S,
    comment: CommentId,
) -> Result<(Comment, Item, Reply), ExternalError> {
    let found = store
        .comment(comment)?
        .filter(|c| c.external.is_some())
        .ok_or(ExternalError::NoComment(comment))?;
    let item = store
        .item(&found.item)?
        .ok_or_else(|| ReviewError::NoItem(found.item.clone()))?;
    reviewable(&item)?;
    let last = store
        .replies(&item.id)?
        .into_iter()
        .rfind(|r| r.comment == comment && !r.is_draft());
    match last {
        Some(reply)
            if reply.author == Author::Agent && reply.status == Some(ReplyStatus::Rejected) =>
        {
            Ok((found, item, reply))
        }
        _ => Err(ExternalError::NotRejected(comment)),
    }
}

#[cfg(test)]
#[path = "rejection_tests.rs"]
mod tests;
