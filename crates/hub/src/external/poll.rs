//! Polls every open github item's pull request for comments, and hands what
//! it holds to the agent once it's due; then for where the pull
//! request stands, so one merged or closed on GitHub closes its item.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use mergegate_core::external::{self, Ingested, Timing};
use mergegate_core::item::{ItemId, ItemStore};

use super::follow::follow;
use super::{changed, now, pull};
use crate::server::AppState;

/// GitHub allows 5000 requests an hour; three lists and the pull request
/// every 30s is 480 per item, and a 304 is free.
const POLL_MS: u64 = 30_000;

pub(crate) async fn run(state: Arc<AppState>) {
    let poll = Duration::from_millis(env_ms("MG_GITHUB_POLL_MS").unwrap_or(POLL_MS));
    let defaults = Timing::default();
    let timing = Timing {
        quiet: env_ms("MG_EXTERNAL_QUIET_MS").unwrap_or(defaults.quiet),
        cap: env_ms("MG_EXTERNAL_CAP_MS").unwrap_or(defaults.cap),
    };
    let mut stopping = state.events.stopping();
    // The last failure per item, so one that persists is logged once.
    let mut failing = Failing::default();
    loop {
        once(&state, timing, &mut failing).await;
        tokio::select! {
            () = tokio::time::sleep(poll) => {}
            _ = stopping.wait_for(|stopping| *stopping) => return,
        }
    }
}

/// Reading comments and reading the pull request fail apart, so one failing
/// while the other recovers isn't logged on every poll.
#[derive(Default)]
struct Failing {
    comments: HashMap<ItemId, String>,
    pull: HashMap<ItemId, String>,
}

async fn once(state: &Arc<AppState>, timing: Timing, failing: &mut Failing) {
    let items = state
        .blocking(|store, _| store.open_items(None))
        .await
        .map(|items| {
            items
                .into_iter()
                .filter(|item| external::pull_of(item).is_some())
                .map(|item| item.id)
                .collect::<Vec<_>>()
        });
    let items = match items {
        Ok(items) => items,
        Err(e) => return tracing::error!("listing items to poll GitHub for: {e}"),
    };
    failing.comments.retain(|id, _| items.contains(id));
    failing.pull.retain(|id, _| items.contains(id));
    for id in items {
        let ingested = match pull(state, &id, false).await {
            Ok(ingested) => {
                failing.comments.remove(&id);
                ingested
            }
            Err(e) => {
                let message = e.to_string();
                if failing.comments.get(&id) != Some(&message) {
                    tracing::warn!("reading {id}'s comments on GitHub: {message}");
                    failing.comments.insert(id.clone(), message);
                }
                Ingested::default()
            }
        };
        // Due even when GitHub can't be read: what's held still goes.
        let released = {
            let id = id.clone();
            state
                .blocking(move |store, _| external::release_due(store, &id, now(), timing))
                .await
        };
        let released = released.unwrap_or_else(|e| {
            tracing::error!("handing {id}'s comments to its agent: {e}");
            None
        });
        changed(state, &id, ingested, released.is_some());
        // After the comments, so those left just before the merge still
        // reach the agent ahead of it.
        match follow(state, &id).await {
            Ok(_) => {
                failing.pull.remove(&id);
            }
            Err(e) => {
                let message = e.to_string();
                if failing.pull.get(&id) != Some(&message) {
                    tracing::warn!("following {id}'s pull request: {message}");
                    failing.pull.insert(id.clone(), message);
                }
            }
        }
    }
}

fn env_ms(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}
