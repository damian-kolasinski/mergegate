//! `POST /api/items/{id}/abandon`: closing an item without merging it, by
//! the operator from the UI or its agent with `mg abandon`.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use mergegate_core::item::{self, Abandon, ItemId, ItemStore};
use mergegate_core::store::Shared;

use crate::reply::Failure;
use crate::server::AppState;
use crate::{branches, feedback, items, merge};

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
    // Not while a merge is landing, which may move what's stacked on it.
    let (item, child) = state
        .stepwise(move |steps, git, github| {
            steps.with(|store| {
                let item = item::abandon(store, git, github, request)?;
                branches::drop(store, git, &item.id);
                // Handed back to its agent; failing to read it mustn't fail
                // the abandon, which happened.
                let child = store.child(&item.id).ok().flatten().map(|c| c.id);
                Ok((item, child))
            })
        })
        .await
        .map_err(items::failure)?;
    // Its parent's stack trail shows it abandoned.
    let stacked: Vec<_> = child.into_iter().chain(item.parent.clone()).collect();
    merge::stacked_changed(&state, &stacked);
    state.events.logged();
    // Starts its attachments' time to live.
    state.prune.notify_one();
    state.events.send(UiEvent::InboxChanged);
    state.events.send(UiEvent::ItemChanged {
        item: item.id.to_string(),
    });
    Ok(Json(items::to_api(&item)))
}
