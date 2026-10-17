//! An item whose pull request was merged or closed on GitHub rather than
//! through the hub. It can't move on there any more, so the item closes the
//! way its pull request went instead of waiting in the inbox for a review
//! that would land nothing.

use super::close::close;
use super::{Item, ItemBackend, ItemError, ItemGit, ItemId, ItemState, ItemStore};
use crate::diff::DiffStore;
use crate::external::ExternalStore;
use crate::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use crate::github::PullStatus;
use crate::highlight::HighlightStore;

/// Why an item abandoned for its pull request says it was.
pub const CLOSED_ON_GITHUB: &str = "Its pull request was closed on GitHub without merging.";

/// Closes `id` the way its pull request `number` went: merged as merged,
/// closed as abandoned, either told to its agent as GitHub's doing. `status`
/// is read without the store, so an item the hub closed meanwhile, or that
/// isn't on that pull request, is left as it is. The closed item, if this
/// closed it.
pub fn follow_pull<S: ItemStore + DiffStore + HighlightStore + FeedbackStore + ExternalStore>(
    store: &mut S,
    git: &impl ItemGit,
    id: &ItemId,
    number: u64,
    status: PullStatus,
) -> Result<Option<Item>, ItemError> {
    let item = store
        .item(id)?
        .ok_or_else(|| ItemError::NoItem(id.clone()))?;
    let on_pull =
        matches!(&item.backend, ItemBackend::Github { pull, .. } if pull.number == number);
    if !item.state.is_open() || !on_pull {
        return Ok(None);
    }
    let (state, source, kind) = match status {
        PullStatus::Open => return Ok(None),
        // An approval held in the hub merged on GitHub is still the
        // operator's merge.
        PullStatus::Merged { commit, head } if head == item.revision.head => {
            let source = match item.state {
                ItemState::Merging => Source::Operator,
                _ => Source::External,
            };
            (ItemState::Merged, source, NoticeKind::Merged { commit })
        }
        // A revision submitted after GitHub merged an earlier head never
        // landed. Told it merged, the agent would delete it with the branch.
        PullStatus::Merged { commit, head } => (
            ItemState::Abandoned,
            Source::External,
            NoticeKind::Abandoned {
                reason: Some(format!(
                    "Its pull request was merged on GitHub as {commit} at {head}, \
                     not at this revision's head {}, so this revision didn't land.",
                    item.revision.head
                )),
            },
        ),
        PullStatus::Closed => (
            ItemState::Abandoned,
            Source::External,
            NoticeKind::Abandoned {
                reason: Some(CLOSED_ON_GITHUB.into()),
            },
        ),
    };
    // Comments still waiting for reviewers to go quiet would never go once
    // the item closes, and one may say why it closed.
    if matches!(
        item.state,
        ItemState::NeedsReview | ItemState::ChangesRequested
    ) {
        store.release(id, item.revision.number)?;
    }
    let notice = Notice {
        revision: item.revision.number,
        source,
        kind,
    };
    close(store, git, &item, state, notice).map(Some)
}

#[cfg(test)]
#[path = "follow_pull_tests.rs"]
mod tests;
