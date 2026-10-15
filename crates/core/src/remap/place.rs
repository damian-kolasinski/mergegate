//! Placing comments written on an earlier revision onto a later one, so a
//! review of the later shows them on the lines they were about (F7, F13).
//!
//! It runs as the item is read: a placement depends on the revision on
//! screen, and costs one cached diff per file, or a diff of two blobs when
//! the base moved (tech-stack §9).

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};

use super::lines::{Mapped, map};
use crate::diff::{Content, DiffGit, DiffStore, Hunk, compute};
use crate::git::GitError;
use crate::interdiff::InterdiffStore;
use crate::item::{ChangedFile, ItemError, ItemId, ItemStore};
use crate::review::{Comment, CommentId, ReviewError, Shown, Side};
use crate::store::StoreError;

/// Blob pairs read from git at once, as at submit.
const BATCH: usize = 64;

/// A file's blob in a comment's revision and in the one it's placed onto.
type BlobPair = (String, String);

/// Where a comment from an earlier revision is on a later one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// On the same lines, unchanged, at these numbers.
    Moved { start_line: u32, line: u32 },
    /// On the lines that hold what's left of its lines and what replaced the
    /// rest.
    Changed { start_line: u32, line: u32 },
    /// Nowhere the later revision's diff shows: its lines are gone, or show
    /// only in a gap the diff leaves out.
    Outdated,
}

#[derive(Debug, thiserror::Error)]
pub enum PlaceError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<PlaceError> for ItemError {
    fn from(error: PlaceError) -> Self {
        match error {
            PlaceError::Git(error) => error.into(),
            PlaceError::Store(error) => error.into(),
        }
    }
}

impl From<PlaceError> for ReviewError {
    fn from(error: PlaceError) -> Self {
        match error {
            PlaceError::Git(error) => error.into(),
            PlaceError::Store(error) => error.into(),
        }
    }
}

/// Places each of `comments` that's on lines of a revision before `onto`;
/// the others have no placement. A file matches by path, so a comment on a
/// file renamed since is outdated.
///
/// A side's lines map through the diff between that side's blobs in the two
/// revisions: the new side follows the agent's changes, the old side the
/// base's.
pub fn place<S: ItemStore + DiffStore + InterdiffStore>(
    store: &S,
    git: &impl DiffGit,
    id: &ItemId,
    onto: u32,
    comments: &[Comment],
) -> Result<BTreeMap<CommentId, Placement>, PlaceError> {
    let earlier: Vec<&Comment> = comments
        .iter()
        .filter(|c| c.revision < onto && c.anchor.is_some())
        .collect();
    if earlier.is_empty() {
        return Ok(BTreeMap::new());
    }
    let target = files(store, id, onto)?;
    let mut revisions: HashMap<u32, HashMap<String, ChangedFile>> = HashMap::new();
    // The blobs of each comment's side in its revision and in `onto`; none
    // when either revision doesn't have the file on that side.
    let mut pairs: Vec<Option<BlobPair>> = Vec::with_capacity(earlier.len());
    for comment in &earlier {
        let anchor = comment.anchor.as_ref().expect("filtered on its anchor");
        let from = match revisions.entry(comment.revision) {
            Entry::Occupied(from) => from.into_mut(),
            Entry::Vacant(from) => from.insert(files(store, id, comment.revision)?),
        };
        let blob = |file: &ChangedFile| match anchor.side {
            Side::Old => file.old_blob.clone(),
            Side::New => file.new_blob.clone(),
        };
        let from = from.get(&anchor.path).and_then(blob);
        let to = target.get(&anchor.path).and_then(blob);
        pairs.push(from.zip(to));
    }
    let diffs = diffs(store, git, id, &pairs)?;
    let mut shown: HashMap<&str, Option<Shown>> = HashMap::new();
    let mut placed = BTreeMap::new();
    for (comment, pair) in earlier.into_iter().zip(&pairs) {
        let anchor = comment.anchor.as_ref().expect("filtered on its anchor");
        let (start_line, line) = (anchor.start_line, anchor.line);
        let mapped = match pair {
            None => Mapped::Gone,
            Some((from, to)) if from == to => Mapped::Same { start_line, line },
            Some(pair) => match &diffs[pair] {
                Some(hunks) => map(hunks, start_line, line),
                // Its lines can't be followed through a change that isn't
                // text or is too large to keep.
                None => Mapped::Gone,
            },
        };
        let (placement, start_line, line) = match mapped {
            Mapped::Same { start_line, line } => {
                (Placement::Moved { start_line, line }, start_line, line)
            }
            Mapped::Changed { start_line, line } => {
                (Placement::Changed { start_line, line }, start_line, line)
            }
            Mapped::Gone => {
                placed.insert(comment.id, Placement::Outdated);
                continue;
            }
        };
        let path = anchor.path.as_str();
        let on = match shown.entry(path) {
            Entry::Occupied(on) => on.into_mut(),
            // `pair` is some, so `onto` has the file.
            Entry::Vacant(on) => on.insert(Shown::read(store, id, onto, &target[path])?),
        };
        let seen = on
            .as_ref()
            .is_some_and(|on| on.shows(anchor.side, start_line, line));
        placed.insert(
            comment.id,
            if seen { placement } else { Placement::Outdated },
        );
    }
    Ok(placed)
}

fn files<S: ItemStore>(
    store: &S,
    id: &ItemId,
    revision: u32,
) -> Result<HashMap<String, ChangedFile>, StoreError> {
    Ok(store
        .revision_files(id, revision)?
        .into_iter()
        .map(|stat| (stat.file.path.clone(), stat.file))
        .collect())
}

/// The hunks between each pair of different blobs, from the diff cache, or
/// computed from staging's blobs when no revision diffs them, as when the
/// base moved; none for a change that isn't text or is too large to keep.
/// Nothing is cached: the pair belongs to no revision's diff.
fn diffs<S: DiffStore>(
    store: &S,
    git: &impl DiffGit,
    id: &ItemId,
    pairs: &[Option<BlobPair>],
) -> Result<HashMap<BlobPair, Option<Vec<Hunk>>>, PlaceError> {
    let hunks = |content: Content| match content {
        Content::Text(hunks) => Some(hunks),
        Content::Binary | Content::TooLarge => None,
    };
    let mut diffs = HashMap::new();
    let mut uncached = Vec::new();
    for pair in pairs.iter().flatten() {
        let (from, to) = pair;
        if from == to || diffs.contains_key(pair) || uncached.contains(&pair) {
            continue;
        }
        match store.diff(Some(from), Some(to))? {
            Some(diff) => {
                diffs.insert(pair.clone(), hunks(diff.content));
            }
            None => uncached.push(pair),
        }
    }
    for batch in uncached.chunks(BATCH) {
        let ids: Vec<&str> = batch
            .iter()
            .flat_map(|(from, to)| [from.as_str(), to.as_str()])
            .collect();
        let mut blobs = git.blobs(id.project(), &ids)?.into_iter();
        for &pair in batch {
            let diff = match (blobs.next().flatten(), blobs.next().flatten()) {
                (Some(from), Some(to)) => hunks(compute(&from, &to).content),
                _ => None,
            };
            diffs.insert(pair.clone(), diff);
        }
    }
    Ok(diffs)
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod tests;
