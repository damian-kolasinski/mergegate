//! Review time: how long the operator was active on an item's review screen,
//! as the UI measures it, and the stats a closed item's `item.merged` or
//! `item.abandoned` carries for its agent's telemetry.
//!
//! Time is split by turn and by revision. A turn is one round of the
//! operator's review: each review they send ends one, the approval included,
//! and the time measured until then is that turn's. Revisions and turns
//! differ: an agent's rebase adds a revision nobody reviewed, and a review
//! answered with replies alone starts a turn on the same revision.
//!
//! Time is recorded only while the item is in review, waiting on the
//! operator or its agent. Time on a merging item would start a turn after
//! the approval that ended the last, and a closed item's totals never
//! change, so its closing event reads the same every time it's read.

use crate::item::{Item, ItemId, ItemState, ItemStore};
use crate::review::Verdict;
use crate::store::StoreError;

pub trait ReviewTimeStore {
    /// What was logged on `id`, one entry per turn and revision with time.
    fn review_time(&self, id: &ItemId) -> Result<Vec<Logged>, StoreError>;

    /// Adds `logged.ms` to its turn's time on its revision.
    fn add_review_time(&mut self, id: &ItemId, logged: Logged) -> Result<(), StoreError>;

    /// The reviews the operator sent on `id`, oldest first: the ends of its
    /// turns.
    fn sent_reviews(&self, id: &ItemId) -> Result<Vec<SentReview>, StoreError>;
}

/// Review time in one turn on one revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Logged {
    /// From 1.
    pub turn: u32,
    pub revision: u32,
    pub ms: u64,
}

/// A review the operator sent, which ended a turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentReview {
    pub revision: u32,
    pub verdict: Verdict,
    pub submitted_at: String,
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
    #[error("{0} is merging")]
    Merging(ItemId),
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
    match item.state {
        ItemState::NeedsReview | ItemState::ChangesRequested => {}
        ItemState::Merging => return Err(ReviewTimeError::Merging(id)),
        ItemState::Merged | ItemState::Abandoned => return Err(ReviewTimeError::Closed(id)),
    }
    if revision == 0 || revision > item.revision.number {
        return Err(ReviewTimeError::NoRevision { id, revision });
    }
    if ms > 0 {
        // The UI reports what it measured before a review goes out, so time
        // logged after one is the next turn's.
        let turn = store.sent_reviews(&id)?.len() as u32 + 1;
        store.add_review_time(&id, Logged { turn, revision, ms })?;
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
    /// All of the item's review time.
    pub review_ms: u64,
    /// Every turn a review ended, oldest first, then the one under way when
    /// the item closed if it had any time.
    pub turns: Vec<TurnStats>,
    /// Every revision, oldest first.
    pub revisions: Vec<RevisionStats>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnStats {
    /// From 1.
    pub number: u32,
    /// The revision the turn's review was sent on; for the turn under way,
    /// the newest it had time on.
    pub revision: u32,
    /// None for the turn under way: the item closed without the operator's
    /// review, abandoned or merged on GitHub.
    pub verdict: Option<Verdict>,
    pub sent_at: Option<String>,
    pub review_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionStats {
    pub number: u32,
    pub submitted_at: String,
    pub review_ms: u64,
}

/// None when no review time was logged on `item`: it closed before the hub
/// measured any, or nobody opened it in the UI. Totals of zero would pass
/// for a review that took no time.
pub fn stats<S: ItemStore + ReviewTimeStore>(
    store: &S,
    item: &Item,
) -> Result<Option<Stats>, StoreError> {
    let logged = store.review_time(&item.id)?;
    if logged.is_empty() {
        return Ok(None);
    }
    let sum = |of: &dyn Fn(&Logged) -> bool| -> u64 {
        logged.iter().filter(|l| of(l)).map(|l| l.ms).sum()
    };
    let mut turns: Vec<_> = store
        .sent_reviews(&item.id)?
        .into_iter()
        .zip(1..)
        .map(|(review, number)| TurnStats {
            number,
            revision: review.revision,
            verdict: Some(review.verdict),
            sent_at: Some(review.submitted_at),
            review_ms: sum(&|l| l.turn == number),
        })
        .collect();
    let under_way = turns.len() as u32 + 1;
    let newest = logged
        .iter()
        .filter(|l| l.turn == under_way)
        .map(|l| l.revision)
        .max();
    if let Some(revision) = newest {
        turns.push(TurnStats {
            number: under_way,
            revision,
            verdict: None,
            sent_at: None,
            review_ms: sum(&|l| l.turn == under_way),
        });
    }
    let revisions = store
        .revisions(&item.id)?
        .into_iter()
        .map(|revision| RevisionStats {
            review_ms: sum(&|l| l.revision == revision.number),
            number: revision.number,
            submitted_at: revision.submitted_at,
        })
        .collect();
    Ok(Some(Stats {
        opened_at: item.created_at.clone(),
        closed_at: item.closed_at.clone(),
        review_ms: sum(&|_| true),
        turns,
        revisions,
    }))
}

#[cfg(test)]
#[path = "review_time_tests.rs"]
mod tests;
