//! Following an item's pull request: one merged or closed on GitHub, not
//! through the hub, closes the item the same way, so it leaves the inbox and
//! its agent hears how it ended.

use std::sync::Arc;

use mergegate_core::external;
use mergegate_core::github::{Github, PullStatus};
use mergegate_core::item::{self, ItemError, ItemId, ItemState, ItemStore};

use crate::server::AppState;
use crate::{branches, merge};

/// Reads where `id`'s pull request stands with the store unlocked, then
/// closes `id` to match. Whether it closed it.
pub(super) async fn follow(state: &Arc<AppState>, id: &ItemId) -> Result<bool, ItemError> {
    let pull = {
        let id = id.clone();
        state
            .blocking(move |store, _| {
                let item = store
                    .item(&id)?
                    .ok_or_else(|| ItemError::NoItem(id.clone()))?;
                let pull = external::pull_of(&item).filter(|_| item.state.is_open());
                let pull = pull.map(|(repo, number)| (repo.clone(), number, item.branch.clone()));
                Ok::<_, ItemError>(pull)
            })
            .await?
    };
    let Some((repo, number, branch)) = pull else {
        return Ok(false);
    };
    let status = {
        let repo = repo.clone();
        state
            .github(move |github| github.pull_status(&repo, number))
            .await?
    };
    if status == PullStatus::Open {
        return Ok(false);
    }
    let closed = {
        let id = id.clone();
        state
            .blocking(move |store, git| {
                let closed = item::follow_pull(store, git, &id, number, status)?;
                if closed.is_some() {
                    branches::drop(store, git, &id);
                }
                Ok::<_, ItemError>(closed)
            })
            .await?
    };
    let Some(closed) = closed else {
        return Ok(false);
    };
    let how = match closed.state {
        ItemState::Merged => "merged",
        _ => "abandoned",
    };
    tracing::info!("{id} {how}: its pull request ended on GitHub");
    // As after the hub's own merge, a branch left behind only clutters
    // GitHub. A closed one stays: whoever closed it may reopen it there.
    if closed.state == ItemState::Merged {
        let deleted = state
            .github(move |github| github.delete_branch(&repo, &branch))
            .await;
        if let Err(e) = deleted {
            tracing::warn!("deleting {id}'s branch on GitHub: {e}");
        }
    }
    merge::changed(state, id);
    Ok(true)
}
