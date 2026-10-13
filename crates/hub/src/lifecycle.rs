//! `GET /api/health` and `POST /api/hub/shutdown`, kept stable across API
//! versions so any `mg` can find and stop any hub.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/hub/shutdown", post(shutdown))
}

async fn health(State(state): State<Arc<AppState>>) -> Json<api::hub::Health> {
    Json(state.health.clone())
}

async fn shutdown(State(state): State<Arc<AppState>>) -> StatusCode {
    state.shutdown.notify_one();
    StatusCode::ACCEPTED
}
