//! `/api/comments/{comment}/drafts`: the operator's draft replies on a sent
//! comment's thread, which the next review sends.

use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{post, put};
use axum::{Json, Router};
use mergegate_core::review::{self, DraftReplyError};

use crate::reply::Failure;
use crate::server::AppState;
use crate::{comments, replies, reviews};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/comments/{comment}/drafts", post(write))
        .route(
            "/api/comments/{comment}/drafts/{reply}",
            put(edit).delete(delete),
        )
}

async fn write(
    State(state): State<Arc<AppState>>,
    comment: Result<Path<String>, PathRejection>,
    request: Result<Json<api::review::WriteDraftReply>, JsonRejection>,
) -> Result<(StatusCode, Json<api::review::Reply>), Failure> {
    let Path(comment) = comment?;
    let Json(request) = request?;
    let comment = comments::comment_id(&comment)?;
    let draft = state
        .blocking(move |store, _| review::write_draft_reply(store, comment, request.body))
        .await
        .map_err(failure)?;
    Ok((StatusCode::CREATED, Json(replies::sent(&draft))))
}

async fn edit(
    State(state): State<Arc<AppState>>,
    path: Result<Path<(String, u64)>, PathRejection>,
    request: Result<Json<api::review::WriteDraftReply>, JsonRejection>,
) -> Result<Json<api::review::Reply>, Failure> {
    let Path((comment, reply)) = path?;
    let Json(request) = request?;
    let comment = comments::comment_id(&comment)?;
    let draft = state
        .blocking(move |store, _| review::edit_draft_reply(store, comment, reply, request.body))
        .await
        .map_err(failure)?;
    Ok(Json(replies::sent(&draft)))
}

async fn delete(
    State(state): State<Arc<AppState>>,
    path: Result<Path<(String, u64)>, PathRejection>,
) -> Result<StatusCode, Failure> {
    let Path((comment, reply)) = path?;
    let comment = comments::comment_id(&comment)?;
    state
        .blocking(move |store, _| review::delete_draft_reply(store, comment, reply))
        .await
        .map_err(failure)?;
    Ok(StatusCode::NO_CONTENT)
}

fn failure(e: DraftReplyError) -> Failure {
    use DraftReplyError as E;
    use StatusCode as S;
    let message = e.to_string();
    let (status, code) = match e {
        // An item that can't take a review can't take a draft either.
        E::Review(e) => return reviews::failure(e),
        E::NoComment(_) => (S::NOT_FOUND, "comment_not_found"),
        E::Empty => (S::BAD_REQUEST, "empty_reply"),
        E::NoDraft { .. } => (S::NOT_FOUND, "draft_not_found"),
        E::Sent(_) => (S::CONFLICT, "reply_sent"),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed"),
    };
    if status.is_server_error() {
        tracing::error!("{message}");
    }
    Failure::new(status, code, message, None)
}
