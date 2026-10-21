//! Telling a held comment GitHub deleted from one a list missed. Lists come a
//! page at a time by offset, so a comment deleted while they're read shifts
//! a later one past a page boundary, and GitHub can answer a short list
//! while it's in trouble. Withdrawing a comment deletes the operator's draft
//! replies with it, so only GitHub saying the comment itself is gone does.

use std::collections::BTreeSet;

use super::ingest::kept;
use crate::github::{Github, GithubError, GithubRepo, PullComment, PullCommentKind};
use crate::item::ItemId;
use crate::review::ReviewStore;
use crate::store::StoreError;

/// A comment as GitHub tells it apart: by kind, then ID.
pub type GithubKey = (PullCommentKind, u64);

/// `id`'s comments waiting for a batch, or for a vote that lapsed: either
/// may still reach the agent.
pub fn held_keys<S: ReviewStore>(
    store: &S,
    id: &ItemId,
) -> Result<BTreeSet<GithubKey>, StoreError> {
    Ok(store
        .comments(id)?
        .into_iter()
        .filter_map(|c| c.external)
        .filter(|external| external.pending || external.lapsed)
        .map(|external| (external.kind, external.github_id))
        .collect())
}

/// Which of `held` that `comments`, as [`fetch`](super::fetch) read them,
/// lack are gone: GitHub has no such comment, or has it with nothing `fetch`
/// keeps. One GitHub still has is added to `comments`, as the list should
/// have had it. One GitHub fails to answer for stays held for a later poll.
pub fn confirm_gone(
    github: &impl Github,
    repo: &GithubRepo,
    number: u64,
    held: &BTreeSet<GithubKey>,
    comments: &mut Vec<PullComment>,
) -> Result<BTreeSet<GithubKey>, GithubError> {
    let listed: BTreeSet<GithubKey> = comments.iter().map(|c| (c.kind, c.id)).collect();
    let mut missing = held.difference(&listed).peekable();
    if missing.peek().is_none() {
        return Ok(BTreeSet::new());
    }
    let viewer = github.viewer()?;
    let mut gone = BTreeSet::new();
    for &(kind, id) in missing {
        match github.pull_comment(repo, number, kind, id) {
            Ok(Some(comment)) if kept(&comment, &viewer) => comments.push(comment),
            Ok(_) => {
                gone.insert((kind, id));
            }
            // The next poll asks again; failing here would hold up the rest
            // of the comments for one GitHub won't answer for.
            Err(_) => {}
        }
    }
    Ok(gone)
}

#[cfg(test)]
#[path = "gone_tests.rs"]
mod tests;
