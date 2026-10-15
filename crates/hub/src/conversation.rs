//! `GET /api/items/{id}/conversation`: an item's comments and replies, which
//! the item screen rereads as the agent answers instead of its whole diff.

use std::sync::Arc;

use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::item::ItemId;
use mergegate_core::review::{self, Comment, Conversation};
use serde::Deserialize;

use crate::reply::Failure;
use crate::server::AppState;
use crate::{comments, items, replies};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/items/{id}/conversation", get(show))
}

#[derive(Deserialize)]
struct ShowQuery {
    /// The revision on screen, which comments from earlier ones are placed
    /// onto; the newest when absent or out of range.
    onto: Option<u32>,
}

async fn show(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
    query: Result<Query<ShowQuery>, QueryRejection>,
) -> Result<Json<api::review::Conversation>, Failure> {
    let Path(id) = id?;
    let Query(ShowQuery { onto }) = query?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let found = {
        let id = id.clone();
        state
            .blocking(move |store, git| review::conversation(store, git, &id, onto))
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
    let comment = |c: &Comment| comments::placed_to_api(c, conversation.placements.get(&c.id));
    api::review::Conversation {
        comments: sent.into_iter().map(comment).collect(),
        drafts: drafts.into_iter().map(comment).collect(),
        replies: sent_replies.into_iter().map(reply).collect(),
        draft_replies: draft_replies.into_iter().map(reply).collect(),
    }
}
