//! `POST /api/items/{id}/approve`, and `POST`/`DELETE /api/items/{id}/merge`
//! to retry or cancel a held merge: the operator merging an item from the UI.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use mergegate_core::feedback::FeedbackStore;
use mergegate_core::item::{Item, ItemId};
use mergegate_core::merge::{self, Approve, Blocked, GithubBackend, Landing, Merge, MergeError};
use mergegate_core::store::StoreError;

use crate::reply::Failure;
use crate::server::AppState;
use crate::{items, projects};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/items/{id}/approve", post(approve))
        .route("/api/items/{id}/merge", post(retry).delete(cancel))
}

async fn approve(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
    request: Result<Json<api::merge::Approve>, JsonRejection>,
) -> Result<Json<api::merge::Merge>, Failure> {
    let Path(id) = id?;
    let Json(request) = request?;
    let id = parse(&id)?;
    let request = Approve {
        id: id.clone(),
        revision: request.revision,
        summary: request.summary,
    };
    let merge = state
        .blocking_github(move |store, git, github| {
            let github = GithubBackend::new(git, github);
            // The hub's git is the local backend.
            merge::approve(store, git, &[git, &github], request)
        })
        .await;
    changed(&state, &id);
    Ok(Json(to_api(merge.map_err(failure)?)))
}

async fn retry(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<api::merge::Merge>, Failure> {
    let Path(id) = id?;
    let id = parse(&id)?;
    let merging = id.clone();
    let merge = state
        .blocking_github(move |store, git, github| {
            let github = GithubBackend::new(git, github);
            merge::retry(store, git, &[git, &github], &merging)
        })
        .await;
    changed(&state, &id);
    Ok(Json(to_api(merge.map_err(failure)?)))
}

async fn cancel(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<api::item::Item>, Failure> {
    let Path(id) = id?;
    let id = parse(&id)?;
    let item = state
        .blocking(move |store, _| merge::cancel(store, &id))
        .await
        .map_err(failure)?;
    changed(&state, &item.id);
    Ok(Json(items::to_api(&item)))
}

/// Every outcome logs an event for the agent and moves the item. So can a
/// failure: one after the approval was recorded must still reach them, and
/// a wake with nothing new only costs a re-read.
fn changed(state: &AppState, id: &ItemId) {
    state.events.logged();
    state.events.send(UiEvent::InboxChanged);
    state.events.send(UiEvent::ItemChanged {
        item: id.to_string(),
    });
}

/// Why `item`'s merge is stuck, for the inbox and the review screen.
pub(crate) fn blocked(
    store: &impl FeedbackStore,
    item: &Item,
) -> Result<Option<api::merge::Blocked>, StoreError> {
    Ok(merge::blocked(store, item)?.map(|blocked| match blocked {
        Blocked::Held { reason } => api::merge::Blocked::Held { reason },
        Blocked::Conflict {
            target_commit,
            paths,
        } => api::merge::Blocked::Conflict {
            target_commit,
            paths,
        },
    }))
}

fn to_api(merge: Merge) -> api::merge::Merge {
    let outcome = match merge.landing {
        Landing::Merged { commit } => api::merge::Outcome::Merged { commit },
        Landing::Held { reason } => api::merge::Outcome::Held { reason },
        Landing::Conflict {
            target_commit,
            paths,
        } => api::merge::Outcome::Conflict {
            target_commit,
            paths,
        },
    };
    api::merge::Merge {
        item: items::to_api(&merge.item),
        outcome,
    }
}

fn parse(id: &str) -> Result<ItemId, Failure> {
    ItemId::parse(id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))
}

fn failure(e: MergeError) -> Failure {
    use MergeError as E;
    use StatusCode as S;
    let e = match e {
        E::Project(e) => return projects::failure(e),
        E::Item(e) => return items::failure(e),
        E::Store(e) => return items::failure(e.into()),
        e => e,
    };
    let (status, code, hint) = match &e {
        E::Project(_) | E::Item(_) | E::Store(_) => unreachable!("answered above"),
        E::NoItem(_) => (S::NOT_FOUND, "item_not_found", Some("mg inbox".to_owned())),
        E::Closed(_) => (S::CONFLICT, "item_closed", None),
        E::Merging(_) => (S::CONFLICT, "item_merging", None),
        E::NotMerging(_) => (S::CONFLICT, "not_merging", None),
        E::Stale { .. } => (S::CONFLICT, "stale_revision", None),
        E::NoBackend { .. } => (S::CONFLICT, "backend_unavailable", None),
    };
    Failure::new(status, code, e.to_string(), hint)
}
