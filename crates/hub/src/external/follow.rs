//! Following an item's pull request: one merged or closed on GitHub, not
//! through the hub, closes the item the same way, so it leaves the inbox and
//! its agent hears how it ended.

use std::sync::Arc;

use mergegate_core::external;
use mergegate_core::github::{Github, PullStatus};
use mergegate_core::item::{self, ItemError, ItemId, ItemState, ItemStore};
use mergegate_core::merge::{GithubBackend, merged_elsewhere};

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
                Ok::<_, ItemError>(pull.map(|(repo, number)| (repo.clone(), number)))
            })
            .await?
    };
    let Some((repo, number)) = pull else {
        return Ok(false);
    };
    let status = state
        .github(move |github| github.pull_status(&repo, number))
        .await?;
    if status == PullStatus::Open {
        return Ok(false);
    }
    let merged_as = match &status {
        PullStatus::Merged { commit, .. } => Some(commit.clone()),
        _ => None,
    };
    let closed = {
        let id = id.clone();
        state
            .blocking_github(move |store, git, github| {
                let Some(closed) = item::follow_pull(store, git, &id, number, status)? else {
                    return Ok::<_, ItemError>(None);
                };
                branches::drop(store, git, &id);
                // As after the hub's own merge, what's stacked on it follows,
                // and a branch left behind only clutters GitHub. A closed one
                // stays: whoever closed it may reopen it there.
                let mut stacked = Vec::new();
                if let (ItemState::Merged, Some(commit)) = (closed.state, &merged_as) {
                    let github = GithubBackend::new(git, github);
                    match merged_elsewhere(store, git, &[git, &github], &id, commit) {
                        Ok(moved) => stacked = moved,
                        Err(e) => tracing::warn!("following {id}'s merge on GitHub: {e}"),
                    }
                } else if let Some(child) = store.child(&id)? {
                    // Told by `follow_pull` that its parent was abandoned.
                    stacked.push(child);
                }
                for child in stacked.iter().filter(|c| c.state == ItemState::Merged) {
                    branches::drop(store, git, &child.id);
                }
                Ok(Some((closed, stacked)))
            })
            .await?
    };
    let Some((closed, stacked)) = closed else {
        return Ok(false);
    };
    let how = match closed.state {
        ItemState::Merged => "merged",
        _ => "abandoned",
    };
    tracing::info!("{id} {how}: its pull request ended on GitHub");
    merge::changed(state, id);
    let stacked: Vec<_> = stacked.into_iter().map(|i| i.id).collect();
    merge::stacked_changed(state, &stacked);
    Ok(true)
}
