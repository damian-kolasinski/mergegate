//! An approval follows the diff, not the revision: a new revision that
//! changes every file exactly as the approved one did lands without a
//! second look, as after a rebase onto a target that moved under a held
//! merge.

use std::collections::BTreeSet;

use super::land::{land, through};
use super::{Merge, MergeBackend, MergeError};
use crate::diff::DiffStore;
use crate::feedback::{EventKind, FeedbackStore, Notice, NoticeKind};
use crate::highlight::HighlightStore;
use crate::interdiff::{Change, InterdiffStore};
use crate::item::{Item, ItemGit, ItemId, ItemState, ItemStore};
use crate::project::ProjectStore;
use crate::review::{NewReview, ReviewStore, Verdict};
use crate::store::StoreError;

/// The revision of `id` whose approval still stands: the operator approved
/// it and hasn't reviewed since, cancelled its merge or seen the item or
/// its parent abandoned. A merge that conflicted or held leaves it standing; the
/// operator's mind didn't change, the target did.
pub fn standing_approval(
    store: &impl FeedbackStore,
    id: &ItemId,
) -> Result<Option<u32>, StoreError> {
    let mut standing = None;
    for event in store.events(id, 0)? {
        match event.kind {
            EventKind::ReviewSubmitted(review) => {
                standing = (review.verdict == Verdict::Approved).then_some(review.revision);
            }
            EventKind::Notice(Notice {
                kind:
                    NoticeKind::MergeCancelled
                    | NoticeKind::Abandoned { .. }
                    | NoticeKind::ParentAbandoned { .. },
                ..
            })
            | EventKind::Override(_) => standing = None,
            EventKind::Notice(_) | EventKind::ExternalComments(_) => {}
        }
    }
    Ok(standing)
}

/// The approved revision whose diff `item`'s newest revision has: every
/// file the newest changes it changes as that one did, trusted or not, and
/// it changes no other. None when there's no interdiff to tell by.
pub fn unchanged_since_approval<S: FeedbackStore + InterdiffStore + ItemStore>(
    store: &S,
    item: &Item,
) -> Result<Option<u32>, StoreError> {
    let newest = item.revision.number;
    let Some(approved) = standing_approval(store, &item.id)?.filter(|&r| r < newest) else {
        return Ok(None);
    };
    let Some(files) = store.interdiff(&item.id, approved, newest)? else {
        return Ok(None);
    };
    if files.iter().any(|f| f.change != Change::Unchanged) {
        return Ok(None);
    }
    // A file the approved revision changed that conflicts on the new base,
    // and that the newest leaves alone, is in no interdiff; the paths each
    // revision changes catch it.
    let paths = |revision| -> Result<BTreeSet<String>, StoreError> {
        Ok(store
            .revision_files(&item.id, revision)?
            .into_iter()
            .map(|stat| stat.file.path)
            .collect())
    };
    Ok((paths(approved)? == paths(newest)?).then_some(approved))
}

/// What carrying the approval did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    /// The revision whose approval carried.
    pub from: u32,
    pub merge: Merge,
}

/// Approves `id`'s newest revision again and lands it, when its diff is that
/// of a revision whose approval stands; none otherwise. The review is the
/// operator's, recorded on their earlier word, so their agent hears of it
/// as of any approval.
pub fn carry<
    S: ProjectStore
        + ItemStore
        + ReviewStore
        + DiffStore
        + InterdiffStore
        + HighlightStore
        + FeedbackStore,
>(
    store: &mut S,
    git: &impl ItemGit,
    backends: &[&dyn MergeBackend],
    id: &ItemId,
) -> Result<Option<Carried>, MergeError> {
    let item = store
        .item(id)?
        .ok_or_else(|| MergeError::NoItem(id.clone()))?;
    if item.state != ItemState::NeedsReview {
        return Ok(None);
    }
    let Some(from) = unchanged_since_approval(store, &item)? else {
        return Ok(None);
    };
    // A review sends every draft, and the operator hasn't finished these.
    let drafting = store.comments(id)?.iter().any(|c| c.is_draft())
        || store.replies(id)?.iter().any(|r| r.is_draft());
    if drafting {
        return Ok(None);
    }
    let backend = through(&item, backends)?;
    let review = NewReview {
        item: id.clone(),
        revision: item.revision.number,
        verdict: Verdict::Approved,
        summary: format!("Approved again: the diff is unchanged since r{from}."),
        moved: Vec::new(),
    };
    store.insert_review(review, ItemState::Merging)?;
    let item = store
        .item(id)?
        .ok_or_else(|| MergeError::NoItem(id.clone()))?;
    let merge = land(store, git, backend, &item)?;
    Ok(Some(Carried { from, merge }))
}

#[cfg(test)]
#[path = "carry_tests.rs"]
mod tests;
