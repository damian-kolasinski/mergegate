//! `POST /api/items/{id}/reviews`: the operator submits a review, which its
//! agent reads as a `review.submitted` event.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use mergegate_core::item::ItemId;
use mergegate_core::review::{self, Review, ReviewError, SubmitReview, Verdict};

use crate::reply::Failure;
use crate::server::AppState;
use crate::{comments, replies};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/items/{id}/reviews", post(submit))
}

async fn submit(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
    request: Result<Json<api::review::SubmitReview>, JsonRejection>,
) -> Result<(StatusCode, Json<api::review::Review>), Failure> {
    let Path(id) = id?;
    let Json(request) = request?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let request = SubmitReview {
        id,
        revision: request.revision,
        verdict: match request.verdict {
            api::review::Verdict::Comment => Verdict::Comment,
            api::review::Verdict::ChangesRequested => Verdict::ChangesRequested,
            api::review::Verdict::Approved => Verdict::Approved,
        },
        summary: request.summary,
    };
    let review = state
        .blocking(move |store, git| review::submit(store, git, request))
        .await
        .map_err(failure)?;
    state.events.logged();
    state.events.send(UiEvent::InboxChanged);
    Ok((StatusCode::CREATED, Json(to_api(&review))))
}

fn to_api(review: &Review) -> api::review::Review {
    api::review::Review {
        id: review.id,
        revision: review.revision,
        verdict: verdict_to_api(review.verdict),
        summary: review.summary.clone(),
        comments: review.comments.iter().map(comments::to_api).collect(),
        replies: review.replies.iter().map(replies::sent).collect(),
        submitted_at: review.submitted_at.clone(),
    }
}

pub(crate) fn verdict_to_api(verdict: Verdict) -> api::review::Verdict {
    match verdict {
        Verdict::Comment => api::review::Verdict::Comment,
        Verdict::ChangesRequested => api::review::Verdict::ChangesRequested,
        Verdict::Approved => api::review::Verdict::Approved,
    }
}

pub(crate) fn failure(e: ReviewError) -> Failure {
    use ReviewError as E;
    use StatusCode as S;
    let (status, code, hint) = match &e {
        E::NoItem(_) => (S::NOT_FOUND, "item_not_found", Some("mg inbox".to_owned())),
        E::NoRevision { .. } => (S::NOT_FOUND, "revision_not_found", None),
        E::Closed(_) => (S::CONFLICT, "item_closed", None),
        E::Merging(_) => (S::CONFLICT, "item_merging", None),
        E::Empty => (S::BAD_REQUEST, "empty_review", None),
        E::Approval => (S::BAD_REQUEST, "invalid_verdict", None),
        E::Stale { newest, .. } => (
            S::CONFLICT,
            "stale_revision",
            Some(format!("show r{newest} and review it")),
        ),
        E::Unplaced { .. } => (S::CONFLICT, "drafts_unplaced", None),
        E::Git(_) => (S::INTERNAL_SERVER_ERROR, "git_failed", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}
