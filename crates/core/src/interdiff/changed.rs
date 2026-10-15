//! What the inbox says changed since the operator's last review.

use std::collections::BTreeSet;

use super::{Change, InterdiffStore};
use crate::feedback::FeedbackStore;
use crate::item::{Item, ItemStore};
use crate::merge::standing_approval;
use crate::review::ReviewStore;
use crate::store::StoreError;
use crate::triage::Tier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangedSince {
    /// The revision last reviewed.
    pub revision: u32,
    /// Files to review that the newest revision changes since then, or no
    /// longer changes.
    pub files: u32,
    /// The operator approved that revision and still stands by it.
    pub approved: bool,
}

/// None until the operator reviewed a revision older than the newest, or
/// when there's no interdiff from it.
pub fn changed_since_review<S: InterdiffStore + ReviewStore + ItemStore + FeedbackStore>(
    store: &S,
    item: &Item,
) -> Result<Option<ChangedSince>, StoreError> {
    let Some(revision) = store
        .last_reviewed(&item.id)?
        .filter(|&r| r < item.revision.number)
    else {
        return Ok(None);
    };
    let Some(files) = store.interdiff(&item.id, revision, item.revision.number)? else {
        return Ok(None);
    };
    // The interdiff isn't triaged: a file's tier is the newest revision's,
    // as the item shows it, and a dropped file is reviewed by default.
    let trusted: BTreeSet<String> = store
        .revision_files(&item.id, item.revision.number)?
        .into_iter()
        .filter(|stat| stat.triage.tier == Tier::Trusted)
        .map(|stat| stat.file.path)
        .collect();
    let files = files
        .iter()
        .filter(|f| f.change != Change::Unchanged && !trusted.contains(&f.stat.file.path))
        .count();
    Ok(Some(ChangedSince {
        revision,
        files: u32::try_from(files).unwrap_or(u32::MAX),
        approved: standing_approval(store, &item.id)? == Some(revision),
    }))
}

#[cfg(test)]
#[path = "changed_tests.rs"]
mod tests;
