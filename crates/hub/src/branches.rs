//! Dropping closed items' branches from staging: right after an item closes,
//! and for every closed item when the hub starts, which catches those that
//! closed before the hub dropped branches, or whose drop failed.

use std::sync::Arc;

use git::HubGit;
use mergegate_core::item::{self, ItemId};
use store_sqlite::Store;

use crate::server::AppState;

/// Run by whoever closed `id`, under the same hold of the store. A failure
/// leaves the branch to the next start's sweep.
pub(crate) fn drop(store: &Store, git: &HubGit, id: &ItemId) {
    if let Err(e) = item::drop_branch(store, git, id) {
        tracing::warn!("dropping {id}'s branch from staging: {e}");
    }
}

/// Runs before the hub takes requests: a push between it reading a closed
/// item's head and deleting the branch could otherwise lose a branch an
/// agent just pushed at that same commit to submit again.
pub(crate) async fn sweep(state: &Arc<AppState>) {
    let failed = state
        .blocking(|store, git| item::drop_closed_branches(store, git))
        .await;
    match failed {
        Ok(failed) => {
            for (id, e) in failed {
                tracing::warn!("dropping {id}'s branch from staging: {e}");
            }
        }
        Err(e) => tracing::error!("dropping closed items' branches from staging: {e}"),
    }
}
