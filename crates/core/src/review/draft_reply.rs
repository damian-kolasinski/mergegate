//! The operator's draft replies: answers on a sent comment's thread, kept on
//! the hub until the next review sends them with its drafts (decision 6).

use super::{Author, CommentId, NewReply, Reply, ReviewError, ReviewStore, reviewable};
use crate::item::ItemStore;
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum DraftReplyError {
    /// The comment's item takes no more feedback.
    #[error(transparent)]
    Review(#[from] ReviewError),
    /// A draft comment has no thread yet; the operator edits it instead.
    #[error("no comment {0}")]
    NoComment(CommentId),
    #[error("a reply needs a body")]
    Empty,
    #[error("{comment} has no draft reply {reply}")]
    NoDraft { comment: CommentId, reply: u64 },
    #[error("reply {0} was sent, so it can't change")]
    Sent(u64),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Drafts the operator's reply on a sent comment's thread.
pub fn write_draft_reply<S: ItemStore + ReviewStore>(
    store: &mut S,
    comment: CommentId,
    body: String,
) -> Result<Reply, DraftReplyError> {
    let found = store
        .comment(comment)?
        .filter(|c| !c.is_draft())
        .ok_or(DraftReplyError::NoComment(comment))?;
    let item = store
        .item(&found.item)?
        .ok_or_else(|| ReviewError::NoItem(found.item.clone()))?;
    reviewable(&item)?;
    let body = body.trim().to_owned();
    if body.is_empty() {
        return Err(DraftReplyError::Empty);
    }
    let reply = NewReply {
        comment,
        item: item.id,
        author: Author::Operator,
        status: None,
        body,
        revision: item.revision.number,
    };
    Ok(store.insert_reply(reply)?)
}

/// Rewrites a draft reply's body; a sent one stays as the agent read it.
pub fn edit_draft_reply<S: ItemStore + ReviewStore>(
    store: &mut S,
    comment: CommentId,
    reply: u64,
    body: String,
) -> Result<Reply, DraftReplyError> {
    let mut draft = draft(store, comment, reply)?;
    // It could never be sent; deleting it is all that's left.
    let item = store
        .item(&draft.item)?
        .ok_or_else(|| ReviewError::NoItem(draft.item.clone()))?;
    reviewable(&item)?;
    let body = body.trim().to_owned();
    if body.is_empty() {
        return Err(DraftReplyError::Empty);
    }
    store.set_draft_reply_body(reply, &body)?;
    draft.body = body;
    Ok(draft)
}

pub fn delete_draft_reply<S: ReviewStore>(
    store: &mut S,
    comment: CommentId,
    reply: u64,
) -> Result<(), DraftReplyError> {
    draft(store, comment, reply)?;
    Ok(store.delete_draft_reply(reply)?)
}

fn draft<S: ReviewStore>(
    store: &S,
    comment: CommentId,
    reply: u64,
) -> Result<Reply, DraftReplyError> {
    // The agent's replies are never drafts, so they read as missing too.
    let found = store
        .reply(reply)?
        .filter(|r| r.comment == comment && r.author == Author::Operator)
        .ok_or(DraftReplyError::NoDraft { comment, reply })?;
    match found.review {
        None => Ok(found),
        Some(_) => Err(DraftReplyError::Sent(reply)),
    }
}

#[cfg(test)]
#[path = "draft_reply_tests.rs"]
mod tests;
