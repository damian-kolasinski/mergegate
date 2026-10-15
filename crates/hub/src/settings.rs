//! `/api/hub/settings`: the hub-wide settings `mg hub set` changes.

use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use mergegate_core::settings::{self, Settings, SettingsError};

use crate::reply::Failure;
use crate::server::AppState;
use crate::tailnet::{self, Listening};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/hub/settings", get(show))
        .route("/api/hub/settings/{key}", put(set))
}

async fn show(
    State(state): State<Arc<AppState>>,
) -> Result<Json<api::settings::Settings>, Failure> {
    let settings = state
        .blocking(|store, _| settings::settings(store))
        .await
        .map_err(failure)?;
    Ok(Json(to_api(&state, settings)))
}

async fn set(
    State(state): State<Arc<AppState>>,
    key: Result<Path<String>, PathRejection>,
    request: Result<Json<api::settings::SetSetting>, JsonRejection>,
) -> Result<Json<api::settings::Settings>, Failure> {
    let Path(key) = key?;
    let Json(request) = request?;
    let settings = state
        .blocking(move |store, _| settings::set(store, &key, &request.value))
        .await
        .map_err(failure)?;
    // A shorter time to live may make attachments due now.
    state.prune.notify_one();
    tailnet::apply(&state, settings.listen).await;
    Ok(Json(to_api(&state, settings)))
}

fn to_api(state: &AppState, settings: Settings) -> api::settings::Settings {
    let tailnet = match state.tailnet.listening() {
        Listening::Off => None,
        Listening::Waiting(problem) => Some(api::settings::Tailnet {
            url: None,
            problem: Some(problem),
        }),
        Listening::Bound(bound) => Some(api::settings::Tailnet {
            url: Some(bound.url.clone()),
            problem: None,
        }),
    };
    api::settings::Settings {
        attachment_ttl: settings.attachment_ttl.to_string(),
        listen: settings.listen.as_str().to_owned(),
        tailnet,
    }
}

fn failure(e: SettingsError) -> Failure {
    use SettingsError as E;
    use StatusCode as S;
    let (status, code) = match &e {
        E::Unknown { .. } => (S::NOT_FOUND, "unknown_setting"),
        E::InvalidValue { .. } => (S::BAD_REQUEST, "invalid_setting"),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed"),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), None)
}
