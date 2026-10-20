//! Review time: how long the operator was active on an item's review screen,
//! per revision, as the UI measures it, and the stats a closed item's
//! `item.merged` or `item.abandoned` carries for its agent's telemetry.
//!
//! Time is recorded only while the item is open. A closed item's totals
//! never change, so its closing event reads the same every time it's read.

use std::collections::BTreeMap;

use crate::item::{Item, ItemId, ItemStore};
use crate::store::StoreError;

pub trait ReviewTimeStore {
    /// `id`'s review time per revision, in milliseconds; a revision no time
    /// was recorded on is absent.
    fn review_time(&self, id: &ItemId) -> Result<BTreeMap<u32, u64>, StoreError>;

    /// Adds `ms` to `revision`'s review time.
    fn add_review_time(&mut self, id: &ItemId, revision: u32, ms: u64) -> Result<(), StoreError>;
}

/// The longest one report may carry. The UI reports every half minute, so
/// a longer one is a client counting wrong, not an operator reviewing.
pub const MAX_REPORT_MS: u64 = 60 * 60 * 1000;

/// Review time the UI measured on one revision since its last report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spent {
    pub id: ItemId,
    pub revision: u32,
    pub ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ReviewTimeError {
    #[error("no item {0}")]
    NoItem(ItemId),
    #[error("{0} is closed")]
    Closed(ItemId),
    #[error("{id} has no revision {revision}")]
    NoRevision { id: ItemId, revision: u32 },
    #[error("{ms} ms is more than one report may carry ({MAX_REPORT_MS} ms)")]
    TooLong { ms: u64 },
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub fn record<S: ItemStore + ReviewTimeStore>(
    store: &mut S,
    spent: Spent,
) -> Result<(), ReviewTimeError> {
    let Spent { id, revision, ms } = spent;
    if ms > MAX_REPORT_MS {
        return Err(ReviewTimeError::TooLong { ms });
    }
    let item = store
        .item(&id)?
        .ok_or_else(|| ReviewTimeError::NoItem(id.clone()))?;
    if !item.state.is_open() {
        return Err(ReviewTimeError::Closed(id));
    }
    if revision == 0 || revision > item.revision.number {
        return Err(ReviewTimeError::NoRevision { id, revision });
    }
    if ms > 0 {
        store.add_review_time(&id, revision, ms)?;
    }
    Ok(())
}

/// How an item's review went, as far as the hub saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stats {
    /// When its first revision was submitted.
    pub opened_at: String,
    /// None while it's open, and for an item closed before the hub kept it.
    pub closed_at: Option<String>,
    /// The revisions' review time, summed.
    pub review_ms: u64,
    /// Every revision, oldest first.
    pub revisions: Vec<RevisionStats>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionStats {
    pub number: u32,
    pub submitted_at: String,
    pub review_ms: u64,
}

pub fn stats<S: ItemStore + ReviewTimeStore>(store: &S, item: &Item) -> Result<Stats, StoreError> {
    let time = store.review_time(&item.id)?;
    let revisions: Vec<_> = store
        .revisions(&item.id)?
        .into_iter()
        .map(|revision| RevisionStats {
            review_ms: time.get(&revision.number).copied().unwrap_or(0),
            number: revision.number,
            submitted_at: revision.submitted_at,
        })
        .collect();
    Ok(Stats {
        opened_at: item.created_at.clone(),
        closed_at: item.closed_at.clone(),
        review_ms: revisions.iter().map(|r| r.review_ms).sum(),
        revisions,
    })
}

#[cfg(test)]
#[path = "review_time_tests.rs"]
mod tests;
