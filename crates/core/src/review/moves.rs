//! Drafts written on an earlier revision go with a review of a later one, on
//! the lines they were about: they move by themselves only when those lines
//! didn't change (F13).

use super::{Comment, CommentId, ReviewError, ReviewStore};
use crate::diff::{DiffGit, DiffStore};
use crate::interdiff::InterdiffStore;
use crate::item::{ItemId, ItemStore};
use crate::remap::{Placement, place};

/// A draft a review sends on its own revision: the same lines at these
/// numbers, so its quote still holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovedDraft {
    pub comment: CommentId,
    pub revision: u32,
    pub start_line: u32,
    pub line: u32,
}

/// The drafts on revisions before `revision` that a review of it moves.
/// A draft whose lines changed, or that the revision's diff doesn't show,
/// stops the review: the operator decides where it goes.
pub(crate) fn move_drafts<S: ItemStore + ReviewStore + DiffStore + InterdiffStore>(
    store: &S,
    git: &impl DiffGit,
    id: &ItemId,
    revision: u32,
) -> Result<Vec<MovedDraft>, ReviewError> {
    let earlier: Vec<Comment> = store
        .comments(id)?
        .into_iter()
        .filter(|c| c.is_draft() && c.revision < revision)
        .collect();
    let placed = place(store, git, id, revision, &earlier)?;
    let mut moved = Vec::new();
    let mut unplaced = Vec::new();
    for draft in &earlier {
        match placed.get(&draft.id) {
            Some(&Placement::Moved { start_line, line }) => moved.push(MovedDraft {
                comment: draft.id,
                revision,
                start_line,
                line,
            }),
            _ => unplaced.push(draft.id),
        }
    }
    if unplaced.is_empty() {
        Ok(moved)
    } else {
        Err(ReviewError::Unplaced {
            id: id.clone(),
            revision,
            drafts: unplaced,
        })
    }
}

#[cfg(test)]
#[path = "moves_tests.rs"]
mod tests;
