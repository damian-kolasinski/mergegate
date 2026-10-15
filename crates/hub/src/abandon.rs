//! `POST /api/items/{id}/abandon`: closing an item without merging it, by
//! the operator from the UI or its agent with `mg abandon`.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use mergegate_core::item::{self, Abandon, ItemId};

use crate::reply::Failure;
use crate::server::AppState;
use crate::{branches, feedback, items};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/items/{id}/abandon", post(abandon))
}

async fn abandon(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
    request: Result<Json<api::item::Abandon>, JsonRejection>,
) -> Result<Json<api::item::Item>, Failure> {
    let Path(id) = id?;
    let Json(request) = request?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let request = Abandon {
        id,
        source: feedback::source_from_api(request.source),
        reason: request.reason,
    };
    let item = state
        .blocking_github(move |store, git, github| {
            let item = item::abandon(store, git, github, request)?;
            branches::drop(store, git, &item.id);
            Ok(item)
        })
        .await
        .map_err(items::failure)?;
    state.events.logged();
    // Starts its attachments' time to live.
    state.prune.notify_one();
    state.events.send(UiEvent::InboxChanged);
    state.events.send(UiEvent::ItemChanged {
        item: item.id.to_string(),
    });
    Ok(Json(items::to_api(&item)))
}
