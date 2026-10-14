//! `PUT /api/items/{id}/viewed`: the reviewer's viewed marks.

use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::put;
use axum::{Json, Router};
use mergegate_core::item::ItemId;
use mergegate_core::viewed::{self, Mark, ViewedError};

use crate::reply::Failure;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/items/{id}/viewed", put(mark))
}

async fn mark(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
    request: Result<Json<api::viewed::MarkViewed>, JsonRejection>,
) -> Result<StatusCode, Failure> {
    let Path(id) = id?;
    let Json(request) = request?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let mark = Mark {
        id,
        revision: request.revision,
        path: request.path,
        viewed: request.viewed,
    };
    state
        .blocking(move |store, _| viewed::mark(store, mark))
        .await
        .map_err(failure)?;
    Ok(StatusCode::NO_CONTENT)
}

fn failure(e: ViewedError) -> Failure {
    use StatusCode as S;
    use ViewedError as E;
    let (status, code, hint) = match &e {
        E::NoItem(_) => (S::NOT_FOUND, "item_not_found", Some("mg inbox".to_owned())),
        E::NoRevision { .. } => (S::NOT_FOUND, "revision_not_found", None),
        E::NoFile { .. } => (S::NOT_FOUND, "file_not_found", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}
