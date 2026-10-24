//! Owner-only snapshots, with the same disk reader available after a crash.

use std::collections::VecDeque;
use std::fs::File;
use std::sync::Arc;

use api::diagnostics::Snapshot;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Extension, Json, Router};
use mergegate_core::device::Trust;
use serde::Deserialize;

use crate::guard::Caller;
use crate::reply::Failure;
use crate::server::AppState;
use crate::{Home, telemetry};

const MAX_RECORDS: usize = 2048;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/diagnostics", get(read))
}

#[derive(Deserialize)]
struct Filter {
    #[serde(default)]
    since: u64,
    #[serde(default)]
    details: bool,
}

async fn read(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<Caller>,
    filter: Result<Query<Filter>, QueryRejection>,
) -> Result<Json<Snapshot>, Failure> {
    if !matches!(caller, Caller::Local | Caller::Device(_, Trust::Owner, _)) {
        return Err(Failure::new(
            StatusCode::FORBIDDEN,
            "diagnostics_forbidden",
            "only the hub owner can extract hub-wide diagnostics",
            None,
        ));
    }
    let Query(filter) = filter?;
    let home = state.health.home.clone();
    let health = state.health.clone();
    let snapshot = tokio::task::spawn_blocking(move || {
        let mut snapshot = local(
            &std::path::PathBuf::from(home),
            filter.since,
            filter.details,
        );
        snapshot.hub_version = Some(health.version);
        snapshot.api_version = Some(health.api_version);
        snapshot.schema_version = Some(health.schema_version);
        snapshot
    })
    .await
    .map_err(|_| {
        Failure::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "diagnostics_failed",
            "collecting diagnostics failed",
            None,
        )
    })?;
    Ok(Json(snapshot))
}

pub fn collect_local(home: &Home, since: u64, details: bool) -> Snapshot {
    local(home.root(), since, details)
}

fn local(home: &std::path::Path, since: u64, details: bool) -> Snapshot {
    let now = telemetry::now();
    let mut records = VecDeque::new();
    let mut warnings = Vec::new();
    let mut truncated = false;
    for index in (0..telemetry::disk::FILES).rev() {
        let path = telemetry::disk::path(&home.join("telemetry"), index);
        match File::open(path) {
            Ok(file) => {
                let result = telemetry::disk::read(file, |mut record| {
                    if !details {
                        record.detail = None;
                    }
                    if record.timestamp >= since.max(now.saturating_sub(telemetry::disk::RETENTION))
                    {
                        if records.len() == MAX_RECORDS {
                            records.pop_front();
                            truncated = true;
                        }
                        records.push_back(record);
                    }
                });
                if result.is_err() {
                    warnings.push("telemetry_read_failed".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => warnings.push("telemetry_read_failed".into()),
        }
    }
    if truncated {
        warnings.push("older_records_omitted".into());
    }
    if records.is_empty() {
        warnings.push("no_telemetry_records".into());
    }
    Snapshot {
        format_version: 1,
        captured_at: now,
        hub_version: records.back().map(|r| r.version.clone()),
        api_version: None,
        schema_version: None,
        records: records.into(),
        warnings,
    }
}
