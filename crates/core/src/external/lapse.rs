//! A bot's held comment the agent's later changes outran: one awaiting the
//! operator's vote on lines the newest revision's diff no longer shows. Bots
//! review every push and say again what still holds, on the lines it now
//! holds for, so a vote on the old one would ask about code that's gone,
//! and hold the batch until the operator gave it. It lapses instead: it
//! stops waiting, and reaches the agent only if they vote it up. A person's
//! comment won't be said again, so it waits for their vote however its lines
//! changed.
//!
//! A bot editing one doesn't make its lines current: the edit holds it
//! again, and the next lapse takes it back.

use super::{ExternalError, ExternalStore};
use crate::diff::{DiffGit, DiffStore};
use crate::interdiff::InterdiffStore;
use crate::item::{ItemId, ItemState, ItemStore};
use crate::remap::{Placement, place};
use crate::review::{Comment, ReviewError, ReviewStore};

/// Lapses item `id`'s bots' comments awaiting a vote that are outdated on
/// its newest revision, while it's open; how many. Run after each
/// [`ingest`](super::ingest), apart from it: placing the comments reads
/// diffs and git, which can fail where bringing comments in didn't.
pub fn lapse<S: ItemStore + DiffStore + InterdiffStore + ReviewStore + ExternalStore>(
    store: &mut S,
    git: &impl DiffGit,
    id: &ItemId,
) -> Result<usize, ExternalError> {
    let item = store
        .item(id)?
        .ok_or_else(|| ReviewError::NoItem(id.clone()))?;
    if matches!(item.state, ItemState::Merged | ItemState::Abandoned) {
        return Ok(0);
    }
    let newest = item.revision.number;
    let awaiting: Vec<Comment> = store
        .comments(&item.id)?
        .into_iter()
        .filter(|c| {
            c.revision < newest
                && c.anchor.is_some()
                && c.external
                    .as_ref()
                    .is_some_and(|e| e.bot && e.awaiting_vote())
        })
        .collect();
    if awaiting.is_empty() {
        return Ok(0);
    }
    let placed = place(store, git, &item.id, newest, &awaiting)?;
    let mut lapsed = 0;
    for comment in &awaiting {
        if placed.get(&comment.id) == Some(&Placement::Outdated) {
            store.lapse_external(comment.id)?;
            lapsed += 1;
        }
    }
    Ok(lapsed)
}

#[cfg(test)]
#[path = "lapse_tests.rs"]
mod tests;
