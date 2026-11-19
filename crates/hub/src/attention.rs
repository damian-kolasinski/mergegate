//! `POST /api/attention`: what a Claude Code session's Stop hook watches
//! between turns, read for all its items at once and waited on for theirs
//! alone, so an idle session costs a request per wait however busy the hub
//! is. Like the event log, it hands nothing over.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Query, State};
use axum::routing::post;
use axum::{Extension, Json, Router};
use mergegate_core::feedback::{self, Watch, Watching};
use mergegate_core::item::ItemId;
use serde::Deserialize;
use tokio::time::Instant;

use crate::feedback::{failure, parse};
use crate::guard::{self, Caller};
use crate::reply::Failure;
use crate::server::AppState;
use crate::{items, presence};

/// The longest one request waits; the hook asks again.
const MAX_WAIT: Duration = Duration::from_secs(3600);

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/attention", post(watch))
}

#[derive(Deserialize)]
struct WatchQuery {
    since: Option<u64>,
    /// Seconds; waiting needs `since`, as a read without it returns at once.
    #[serde(default)]
    wait: u64,
}

async fn watch(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<Caller>,
    query: Result<Query<WatchQuery>, QueryRejection>,
    request: Result<Json<api::attention::Watch>, JsonRejection>,
) -> Result<Json<api::attention::Watching>, Failure> {
    let Query(query) = query?;
    let Json(request) = request?;
    let watch = Watch {
        device: match &caller {
            Caller::Local => None,
            Caller::Device(device, ..) => Some(device.name.clone()),
        },
        worktree: request.worktree,
        items: request
            .items
            .into_iter()
            .map(|item| Ok((parse(&item.id)?, item.worktree)))
            .collect::<Result<_, Failure>>()?,
    };
    let deadline = Instant::now() + Duration::from_secs(query.wait).min(MAX_WAIT);
    let mut log = state.events.log();
    let mut stopping = state.events.stopping();
    let mut admissions = caller.admissions(&state);
    let mut last = query.wait == 0 || query.since.is_none();
    // The review screen shows each watched item's agent waiting while this
    // does, and for presence's grace after, by when the hook asks again.
    let mut present: HashMap<ItemId, presence::Reading> = HashMap::new();
    loop {
        log.mark_unchanged();
        let watching = {
            let watch = watch.clone();
            state
                .blocking(move |store, _| feedback::watch(store, &watch, query.since))
                .await
                .map_err(|e| failure(e.into()))?
        };
        present.retain(|id, _| watching.items.iter().any(|w| &w.item.id == id));
        for watched in &watching.items {
            present
                .entry(watched.item.id.clone())
                .or_insert_with(|| presence::reading(&state, &watched.item.id));
        }
        if last || watching.changed {
            return Ok(Json(to_api(watching)));
        }
        // The store stays unlocked while this waits.
        let mut readmit = false;
        tokio::select! {
            changed = log.changed() => last = changed.is_err(),
            () = tokio::time::sleep_until(deadline) => last = true,
            _ = stopping.wait_for(|stopping| *stopping) => last = true,
            () = admissions.changed() => readmit = true,
        }
        // A device denied while it waits gets the refusal, not its items.
        if readmit {
            guard::recheck(&state, &caller).await?;
        }
    }
}

fn to_api(watching: Watching) -> api::attention::Watching {
    api::attention::Watching {
        items: watching
            .items
            .into_iter()
            .map(|watched| api::attention::Watched {
                id: watched.item.id.to_string(),
                name: watched.item.name().to_string(),
                state: items::state_to_api(watched.item.state),
                cursor: watched.cursor,
            })
            .collect(),
        cursor: watching.cursor,
    }
}
