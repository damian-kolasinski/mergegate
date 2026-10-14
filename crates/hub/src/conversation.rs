//! `GET /api/items/{id}/conversation`: an item's comments and replies, which
//! the item screen rereads as the agent answers instead of its whole diff.

use std::sync::Arc;

use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::item::ItemId;
use mergegate_core::review::{self, Conversation};

use crate::reply::Failure;
use crate::server::AppState;
use crate::{comments, items, replies};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/items/{id}/conversation", get(show))
}

async fn show(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<api::review::Conversation>, Failure> {
    let Path(id) = id?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let found = {
        let id = id.clone();
        state
            .blocking(move |store, _| review::conversation(store, &id))
            .await
            .map_err(|e| items::failure(e.into()))?
    };
    let conversation = found.ok_or_else(|| {
        Failure::new(
            StatusCode::NOT_FOUND,
            "item_not_found",
            format!("no item {id}"),
            Some("mg inbox".into()),
        )
    })?;
    Ok(Json(to_api(&conversation)))
}

/// Drafts apart from what a review sent, as the UI shows them apart.
pub(crate) fn to_api(conversation: &Conversation) -> api::review::Conversation {
    let (drafts, sent): (Vec<_>, Vec<_>) = conversation.comments.iter().partition(|c| c.is_draft());
    let (draft_replies, sent_replies): (Vec<_>, Vec<_>) =
        conversation.replies.iter().partition(|r| r.is_draft());
    let reply = |reply| replies::to_api(reply, conversation.newest);
    api::review::Conversation {
        comments: sent.into_iter().map(comments::to_api).collect(),
        drafts: drafts.into_iter().map(comments::to_api).collect(),
        replies: sent_replies.into_iter().map(reply).collect(),
        draft_replies: draft_replies.into_iter().map(reply).collect(),
    }
}
