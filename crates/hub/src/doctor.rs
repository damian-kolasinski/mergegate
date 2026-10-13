//! `GET /api/doctor`: the checks of `mg doctor` that only the hub can run.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::doctor::{Check, CheckStatus};

use crate::reply::Failure;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/doctor", get(doctor))
}

async fn doctor(State(state): State<Arc<AppState>>) -> Result<Json<api::doctor::Report>, Failure> {
    let checks = state
        .blocking(|store, git| mergegate_core::doctor::checks(store, git))
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
    Ok(Json(api::doctor::Report {
        checks: checks.into_iter().map(to_api).collect(),
    }))
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
