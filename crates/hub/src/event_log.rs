//! `GET /api/events`: the whole hub's event log, which `mg events --follow`
//! streams to the operator agent. Unlike an item's feedback it hands nothing
//! over: no cursor moves, no held comments go out, and no wake is settled.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Extension, Json, Router};
use mergegate_core::feedback::{self, Log};
use serde::Deserialize;
use tokio::time::Instant;

use crate::feedback::{event_to_api, failure};
use crate::guard::{self, Caller};
use crate::reply::Failure;
use crate::server::AppState;

/// The longest one request waits; `mg events --follow` asks again.
const MAX_WAIT: Duration = Duration::from_secs(3600);

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/events", get(read))
}

#[derive(Deserialize)]
struct ReadQuery {
    since: Option<u64>,
    /// Seconds; waiting needs `since`, as a read without it returns at once.
    #[serde(default)]
    wait: u64,
}

async fn read(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<Caller>,
    query: Result<Query<ReadQuery>, QueryRejection>,
) -> Result<Json<api::feedback::Log>, Failure> {
    let Query(query) = query?;
    let deadline = Instant::now() + Duration::from_secs(query.wait).min(MAX_WAIT);
    let mut logged = state.events.log();
    let mut stopping = state.events.stopping();
    let mut admissions = caller.admissions(&state);
    let mut last = query.wait == 0 || query.since.is_none();
    loop {
        logged.mark_unchanged();
        let read = state
            .blocking(move |store, _| feedback::log(store, query.since))
            .await
            .map_err(|e| failure(e.into()))?;
        if last || !read.events.is_empty() {
            return Ok(Json(to_api(read)));
        }
        // The store stays unlocked while this waits.
        let mut readmit = false;
        tokio::select! {
            changed = logged.changed() => last = changed.is_err(),
            () = tokio::time::sleep_until(deadline) => last = true,
            _ = stopping.wait_for(|stopping| *stopping) => last = true,
            () = admissions.changed() => readmit = true,
        }
        // A device denied while it follows gets the refusal, not events.
        if readmit {
            guard::recheck(&state, &caller).await?;
        }
    }
}

fn to_api(read: Log) -> api::feedback::Log {
    api::feedback::Log {
        events: read
            .events
            .into_iter()
            .map(|(item, event)| event_to_api(&item.id, &item, event))
            .collect(),
        cursor: read.cursor,
    }
}
