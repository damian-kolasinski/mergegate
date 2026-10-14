//! `/api/items/{id}/drafts`: the operator's draft comments, which the next
//! review sends.

use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{post, put};
use axum::{Json, Router};
use mergegate_core::item::ItemId;
use mergegate_core::review::{self, Comment, CommentId, DraftError, Side, WriteDraft};

use crate::reply::Failure;
use crate::reviews;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/items/{id}/drafts", post(write))
        .route("/api/items/{id}/drafts/{comment}", put(edit).delete(delete))
}

async fn write(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
    request: Result<Json<api::review::WriteDraft>, JsonRejection>,
) -> Result<(StatusCode, Json<api::review::Comment>), Failure> {
    let Path(id) = id?;
    let Json(request) = request?;
    let request = WriteDraft {
        id: item_id(&id)?,
        revision: request.revision,
        path: request.path,
        side: match request.side {
            api::review::Side::Old => Side::Old,
            api::review::Side::New => Side::New,
        },
        start_line: request.start_line,
        line: request.line,
        body: request.body,
    };
    let draft = state
        .blocking(move |store, git| review::write_draft(store, git, request))
        .await
        .map_err(failure)?;
    Ok((StatusCode::CREATED, Json(to_api(&draft))))
}

async fn edit(
    State(state): State<Arc<AppState>>,
    path: Result<Path<(String, String)>, PathRejection>,
    request: Result<Json<api::review::EditDraft>, JsonRejection>,
) -> Result<Json<api::review::Comment>, Failure> {
    let Path((id, comment)) = path?;
    let Json(request) = request?;
    let (id, comment) = (item_id(&id)?, comment_id(&comment)?);
    let draft = state
        .blocking(move |store, _| review::edit_draft(store, &id, comment, request.body))
        .await
        .map_err(failure)?;
    Ok(Json(to_api(&draft)))
}

async fn delete(
    State(state): State<Arc<AppState>>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<StatusCode, Failure> {
    let Path((id, comment)) = path?;
    let (id, comment) = (item_id(&id)?, comment_id(&comment)?);
    state
        .blocking(move |store, _| review::delete_draft(store, &id, comment))
        .await
        .map_err(failure)?;
    Ok(StatusCode::NO_CONTENT)
}

fn item_id(id: &str) -> Result<ItemId, Failure> {
    ItemId::parse(id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))
}

pub(crate) fn comment_id(id: &str) -> Result<CommentId, Failure> {
    CommentId::parse(id).ok_or_else(|| {
        Failure::new(
            StatusCode::BAD_REQUEST,
            "invalid_comment",
            format!("{id:?} isn't a comment ID, as in c_17"),
            None,
        )
    })
}

fn failure(e: DraftError) -> Failure {
    use DraftError as E;
    use StatusCode as S;
    let message = e.to_string();
    let (status, code) = match e {
        // An item that can't take a review can't take a draft either.
        E::Review(e) => return reviews::failure(e),
        E::Empty => (S::BAD_REQUEST, "empty_comment"),
        E::NoFile { .. } => (S::NOT_FOUND, "file_not_found"),
        E::NoLines { .. } => (S::BAD_REQUEST, "invalid_lines"),
        E::NotText(_) => (S::BAD_REQUEST, "not_text"),
        E::NoDraft { .. } => (S::NOT_FOUND, "draft_not_found"),
        E::Sent(_) => (S::CONFLICT, "comment_sent"),
        E::Git(_) => (S::INTERNAL_SERVER_ERROR, "git_failed"),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed"),
    };
    if status.is_server_error() {
        tracing::error!("{message}");
    }
    Failure::new(status, code, message, None)
}

pub(crate) fn to_api(comment: &Comment) -> api::review::Comment {
    api::review::Comment {
        id: comment.id.to_string(),
        revision: comment.revision,
        path: comment.path.clone(),
        side: match comment.side {
            Side::Old => api::review::Side::Old,
            Side::New => api::review::Side::New,
        },
        start_line: comment.start_line,
        line: comment.line,
        body: comment.body.clone(),
        quote: comment.quote.clone(),
    }
}
