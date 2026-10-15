//! `POST /api/items/{id}/priority`: the operator, or their agent with
//! `mg prioritize`, ranking an open item in the queue.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use mergegate_core::item::{self, ItemId, Prioritize, Priority};

use crate::items;
use crate::reply::Failure;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/items/{id}/priority", post(prioritize))
}

async fn prioritize(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
    request: Result<Json<api::item::Prioritize>, JsonRejection>,
) -> Result<Json<api::item::Item>, Failure> {
    let Path(id) = id?;
    let Json(request) = request?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let request = Prioritize {
        id,
        priority: request.priority.map(from_api),
        note: request.note,
    };
    let item = state
        .blocking(move |store, _| item::prioritize(store, request))
        .await
        .map_err(items::failure)?;
    state.events.send(UiEvent::InboxChanged);
    state.events.send(UiEvent::ItemChanged {
        item: item.id.to_string(),
    });
    Ok(Json(items::to_api(&item)))
}

pub(crate) fn from_api(priority: api::item::Priority) -> Priority {
    match priority {
        api::item::Priority::P0 => Priority::P0,
        api::item::Priority::P1 => Priority::P1,
        api::item::Priority::P2 => Priority::P2,
        api::item::Priority::P3 => Priority::P3,
    }
}

pub(crate) fn to_api(priority: Priority) -> api::item::Priority {
    match priority {
        Priority::P0 => api::item::Priority::P0,
        Priority::P1 => api::item::Priority::P1,
        Priority::P2 => api::item::Priority::P2,
        Priority::P3 => api::item::Priority::P3,
    }
}
