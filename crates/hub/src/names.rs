//! `GET /api/names/{name}`: the item a name stands for, which `mg` looks up
//! before acting on an item, so agents can use the name they're shown.

use std::sync::Arc;

use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::item::{self, ItemId};

use crate::items;
use crate::reply::Failure;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/names/{name}", get(show))
}

async fn show(
    State(state): State<Arc<AppState>>,
    name: Result<Path<String>, PathRejection>,
) -> Result<Json<api::item::Item>, Failure> {
    let Path(name) = name?;
    let name = ItemId::parse(&name)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let found = {
        let name = name.clone();
        state
            .blocking(move |store, _| item::named(store, &name))
            .await
            .map_err(|e| items::failure(e.into()))?
    };
    let found = found.ok_or_else(|| {
        Failure::new(
            StatusCode::NOT_FOUND,
            "item_not_found",
            format!("no item {name}"),
            Some("mg inbox".into()),
        )
    })?;
    Ok(Json(items::to_api(&found)))
}
