//! `/api/items` and `/api/inbox`: registering pushed work and the review
//! queue.

use std::sync::Arc;

use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use mergegate_core::item::{self, ItemError, Submit};

use crate::projects;
use crate::reply::Failure;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/items", post(submit))
        .route("/api/inbox", get(inbox))
}

async fn submit(
    State(state): State<Arc<AppState>>,
    request: Result<Json<api::item::Submit>, JsonRejection>,
) -> Result<(StatusCode, Json<api::item::Submitted>), Failure> {
    let Json(request) = request?;
    let request = Submit {
        project: request.project,
        branch: request.branch,
        head: request.head,
        title: request.title,
    };
    let submitted = state
        .blocking(move |store, git| item::submit(store, git, request))
        .await
        .map_err(failure)?;
    let opened = submitted.new_revision && submitted.item.revision.number == 1;
    let status = if opened {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((
        status,
        Json(api::item::Submitted {
            item: submitted.item.to_api(),
            new_revision: submitted.new_revision,
        }),
    ))
}

async fn inbox(State(state): State<Arc<AppState>>) -> Result<Json<api::item::Inbox>, Failure> {
    let inbox = state
        .blocking(|store, _| item::inbox(store))
        .await
        .map_err(|e| failure(e.into()))?;
    Ok(Json(inbox))
}

fn failure(e: ItemError) -> Failure {
    use ItemError as E;
    use StatusCode as S;
    let e = match e {
        E::Project(e) => return projects::failure(e),
        e => e,
    };
    let (status, code, hint) = match &e {
        E::Project(_) => unreachable!("answered above"),
        E::InvalidBranch(_) => (S::BAD_REQUEST, "invalid_branch", None),
        E::InvalidCommit(_) => (S::BAD_REQUEST, "invalid_commit", None),
        E::TargetBranch { .. } => (
            S::BAD_REQUEST,
            "target_branch",
            Some("git switch -c NAME".to_owned()),
        ),
        // mg submit pushes before it registers.
        E::NotPushed { .. } | E::NoBranch(_) => {
            (S::CONFLICT, "not_pushed", Some("mg submit".to_owned()))
        }
        E::Unrelated { .. } => (S::BAD_REQUEST, "unrelated_history", None),
        E::NothingToSubmit { .. } => (S::BAD_REQUEST, "nothing_to_submit", None),
        E::Git(_) => (S::INTERNAL_SERVER_ERROR, "git_failed", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}
