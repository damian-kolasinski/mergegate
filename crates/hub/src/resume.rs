//! `GET /api/items/{id}/resume`: what `mg resume` hands a fresh agent session.
//! It acknowledges nothing.

use std::sync::Arc;

use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::item::ItemId;
use mergegate_core::resume::{self, Resume};
use mergegate_core::store::StoreError;

use crate::feedback::{self, Related};
use crate::reply::Failure;
use crate::server::AppState;
use crate::{comments, items, replies, reviews};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/items/{id}/resume", get(show))
}

async fn show(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<api::resume::Resume>, Failure> {
    let Path(id) = id?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let found = {
        let id = id.clone();
        state
            .blocking(move |store, _| {
                let Some(found) = resume::resume(store, &id)? else {
                    return Ok(None);
                };
                let related =
                    feedback::relate(store, found.pending.iter().map(|e| (&found.item, e)))?;
                Ok(Some((found, related)))
            })
            .await
            .map_err(|e: StoreError| items::failure(e.into()))?
    };
    let (found, related) = found.ok_or_else(|| {
        Failure::new(
            StatusCode::NOT_FOUND,
            "item_not_found",
            format!("no item {id}"),
            Some("mg inbox".into()),
        )
    })?;
    Ok(Json(to_api(found, &related)))
}

fn to_api(found: Resume, related: &Related) -> api::resume::Resume {
    let newest = found.item.revision.number;
    let item = &found.item;
    let pending = found
        .pending
        .map(|event| feedback::event_to_api(&item.id, item, event, related));
    api::resume::Resume {
        item: items::to_api(&found.item),
        worktree: found.worktree,
        wake: found.wake,
        review: found.review.map(|review| api::resume::LastReview {
            revision: review.revision,
            verdict: reviews::verdict_to_api(review.verdict),
            summary: review.summary,
        }),
        open: found
            .open
            .iter()
            .map(|thread| api::resume::Thread {
                comment: comments::to_api(&thread.comment),
                replies: thread
                    .replies
                    .iter()
                    .map(|reply| replies::to_api(reply, newest))
                    .collect(),
            })
            .collect(),
        pending,
    }
}
