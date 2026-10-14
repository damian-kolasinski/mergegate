//! `POST /api/comments/{comment}/replies`: the agent answers a sent comment,
//! and open UIs show it on the comment's thread.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use mergegate_core::review::{self, Author, Reply, ReplyError, ReplyStatus, WriteReply};

use crate::reply::Failure;
use crate::server::AppState;
use crate::{comments, reviews};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/comments/{comment}/replies", post(write))
}

async fn write(
    State(state): State<Arc<AppState>>,
    comment: Result<Path<String>, PathRejection>,
    request: Result<Json<api::review::WriteReply>, JsonRejection>,
) -> Result<(StatusCode, Json<api::review::Reply>), Failure> {
    let Path(comment) = comment?;
    let Json(request) = request?;
    let request = WriteReply {
        comment: comments::comment_id(&comment)?,
        status: match request.status {
            api::review::ReplyStatus::Addressed => ReplyStatus::Addressed,
            api::review::ReplyStatus::Rejected => ReplyStatus::Rejected,
            api::review::ReplyStatus::Question => ReplyStatus::Question,
        },
        body: request.body,
    };
    let reply = state
        .blocking(move |store, _| review::reply(store, request))
        .await
        .map_err(|e| failure(e, &comment))?;
    // Not an event for the agent: it wrote the reply.
    state.events.send(UiEvent::ItemChanged {
        item: reply.item.to_string(),
    });
    // Written on the item's newest revision, so nothing has addressed it yet.
    let newest = reply.revision;
    Ok((StatusCode::CREATED, Json(to_api(&reply, newest))))
}

fn failure(e: ReplyError, comment: &str) -> Failure {
    use ReplyError as E;
    use StatusCode as S;
    let message = e.to_string();
    let (status, code, hint) = match e {
        // An item that takes no review takes no replies either.
        E::Review(e) => return reviews::failure(e),
        E::NoComment(_) => (
            S::NOT_FOUND,
            "comment_not_found",
            Some("mg feedback ITEM --since 0".to_owned()),
        ),
        E::NeedsBody(status) => (
            S::BAD_REQUEST,
            "empty_reply",
            Some(format!(
                "mg reply {} --status {} --body WHY",
                comment,
                status.as_str()
            )),
        ),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{message}");
    }
    Failure::new(status, code, message, hint)
}

/// An operator's reply a review sent, which no revision addresses.
pub(crate) fn sent(reply: &Reply) -> api::review::Reply {
    to_api(reply, reply.revision)
}

/// `newest` is the item's newest revision, which tells whether a later one
/// addressed the reply.
pub(crate) fn to_api(reply: &Reply, newest: u32) -> api::review::Reply {
    api::review::Reply {
        id: reply.id,
        comment: reply.comment.to_string(),
        author: match reply.author {
            Author::Agent => api::review::Author::Agent,
            Author::Operator => api::review::Author::Operator,
        },
        status: reply.status.map(|status| match status {
            ReplyStatus::Addressed => api::review::ReplyStatus::Addressed,
            ReplyStatus::Rejected => api::review::ReplyStatus::Rejected,
            ReplyStatus::Question => api::review::ReplyStatus::Question,
        }),
        body: reply.body.clone(),
        addressed_in: reply.addressed_in(newest),
        overrides: reply.overrides,
        created_at: reply.created_at.clone(),
    }
}
