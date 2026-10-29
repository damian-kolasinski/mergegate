//! `GET /api/wakes`: hands a device's `mg wakes` runner the wake commands its
//! agents left, as the hub's own runner in [`crate::wake`] starts the ones
//! left from the hub's machine.
//!
//! A wake handed over is recorded as woken before the response is sent: one
//! whose response is lost doesn't run, as a local command that fails isn't
//! retried.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Extension, Json, Router};
use mergegate_core::agent::{self, DueWake};
use mergegate_core::item::ItemId;
use serde::Deserialize;
use tokio::time::Instant;

use crate::guard::{self, Caller};
use crate::reply::Failure;
use crate::server::AppState;
use crate::wake;

/// The longest one poll waits; the runner polls again.
const MAX_WAIT: Duration = Duration::from_secs(300);

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/wakes", get(poll))
}

#[derive(Deserialize)]
struct PollQuery {
    /// Seconds.
    #[serde(default)]
    wait: u64,
    #[serde(default)]
    running: String,
}

async fn poll(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<Caller>,
    query: Result<Query<PollQuery>, QueryRejection>,
) -> Result<Json<api::wake::Due>, Failure> {
    let Query(query) = query?;
    let Caller::Device(device, ..) = &caller else {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "wakes_local",
            "the hub runs the wake commands left on its own machine",
            None,
        ));
    };
    let running = query
        .running
        .split(',')
        .filter(|id| !id.is_empty())
        .map(ItemId::parse)
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let name = device.name.clone();
    let _polling = state.wakes.polling(&name, &running);
    let deadline = Instant::now() + Duration::from_secs(query.wait).min(MAX_WAIT);
    let mut log = state.events.log();
    let mut changes = state.wakes.changes();
    let mut stopping = state.events.stopping();
    let mut admissions = caller.admissions(&state);
    let mut last = query.wait == 0;
    loop {
        log.mark_unchanged();
        changes.mark_unchanged();
        let starts = {
            let state = Arc::clone(&state);
            let name = name.clone();
            state
                .clone()
                .blocking(move |store, _| wake::take(store, &state.wakes, Some(&name)))
                .await
                .map_err(|e| {
                    tracing::error!("reading due wakes: {e}");
                    Failure::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "store_failed",
                        e.to_string(),
                        None,
                    )
                })?
        };
        if !starts.is_empty() || last {
            for wake in &starts {
                tracing::info!(
                    event_code = "wake_handed_off",
                    "handing {} at cursor {} to {name}",
                    wake.item,
                    wake.cursor
                );
            }
            return Ok(Json(api::wake::Due {
                wakes: starts.into_iter().filter_map(to_api).collect(),
            }));
        }
        let mut readmit = false;
        tokio::select! {
            changed = log.changed() => last = changed.is_err(),
            _ = changes.changed() => {}
            () = tokio::time::sleep_until(deadline) => last = true,
            _ = stopping.wait_for(|stopping| *stopping) => last = true,
            () = admissions.changed() => readmit = true,
        }
        // A device denied while it waits gets the refusal, not wakes.
        if readmit {
            guard::recheck(&state, &caller).await?;
        }
    }
}

/// None without a worktree, which a submit from a device always records.
fn to_api(wake: DueWake) -> Option<api::wake::Wake> {
    Some(api::wake::Wake {
        command: agent::wake_command(&wake.command, &wake.name),
        id: wake.item.to_string(),
        item: wake.name.to_string(),
        worktree: wake.worktree?,
        cursor: wake.cursor,
    })
}
