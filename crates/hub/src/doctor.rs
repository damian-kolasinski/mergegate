//! `GET /api/doctor`: the checks of `mg doctor` that only the hub can run.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::doctor::{Check, CheckStatus};
use mergegate_core::highlight::{Highlighter, PackSource};
use mergegate_core::project::ProjectStore;

use crate::reply::Failure;
use crate::server::AppState;
use crate::tailnet::Listening;
use crate::ui;

pub(crate) mod firewall;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/doctor", get(doctor))
}

async fn doctor(State(state): State<Arc<AppState>>) -> Result<Json<api::doctor::Report>, Failure> {
    let languages = Arc::clone(&state);
    let checks = state
        .blocking_github(move |store, git, github| {
            let mut checks = mergegate_core::doctor::checks(store, git, github)?;
            checks.extend(mergegate_core::doctor::languages(
                git,
                &store.projects()?,
                &languages.known_packs,
                &languages.languages.installed(),
                languages.releases.releases(),
            ));
            Ok::<_, mergegate_core::store::StoreError>(checks)
        })
        .await
        .map_err(|e| {
            tracing::error!("{e}");
            Failure::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_failed",
                e.to_string(),
                None,
            )
        })?;
    let tailnet = tailnet_check(&state).await;
    // Only a listener on the tailnet takes connections the firewall filters.
    let firewall = match state.tailnet.listening() {
        Listening::Bound(bound) if bound.ip.is_some() => firewall::check().await,
        _ => None,
    };
    let checks = std::iter::once(ui::check())
        .chain(tailnet)
        .chain(firewall)
        .chain(checks);
    Ok(Json(api::doctor::Report {
        checks: checks.map(to_api).collect(),
    }))
}

/// Only with `listen` set to `tailnet`. A listener once bound stays, so
/// this asks Tailscale whether peers can still reach it.
async fn tailnet_check(state: &AppState) -> Option<Check> {
    let restart = || Some("restart the hub: mg hub stop, then mg hub start".to_owned());
    let (status, message, fix) = match state.tailnet.listening() {
        Listening::Off => return None,
        Listening::Bound(bound) => match state.tailnet.status().await {
            Err(e) => (
                CheckStatus::Warn,
                format!("listening on the tailnet at {}, but {e}", bound.url),
                Some("start Tailscale and log in".to_owned()),
            ),
            Ok(now) => match bound.ip {
                Some(ip) if !now.me.ips.contains(&ip) => (
                    CheckStatus::Warn,
                    format!("listening on {ip}, which is no longer this machine's tailnet address"),
                    restart(),
                ),
                _ => (
                    CheckStatus::Ok,
                    format!("listening on the tailnet at {}", bound.url),
                    None,
                ),
            },
        },
        Listening::Waiting(problem) => (
            CheckStatus::Warn,
            format!("not listening on the tailnet yet, retrying: {problem}"),
            Some("start Tailscale and log in, or: mg hub set listen local".to_owned()),
        ),
    };
    Some(Check {
        name: "tailnet".into(),
        status,
        message,
        fix,
    })
}

fn to_api(check: Check) -> api::doctor::Check {
    api::doctor::Check {
        name: check.name,
        status: match check.status {
            CheckStatus::Ok => api::doctor::CheckStatus::Ok,
            CheckStatus::Warn => api::doctor::CheckStatus::Warn,
            CheckStatus::Fail => api::doctor::CheckStatus::Fail,
        },
        message: check.message,
        fix: check.fix,
    }
}
