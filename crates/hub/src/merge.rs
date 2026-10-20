//! `POST /api/items/{id}/approve`, and `POST`/`DELETE /api/items/{id}/merge`
//! to retry or cancel a held merge: the operator merging an item from the UI.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use backend_github::GithubClient;
use git::HubGit;
use mergegate_core::feedback::FeedbackStore;
use mergegate_core::item::{Item, ItemId, ItemState, ItemStore};
use mergegate_core::merge::{
    self, Approve, Blocked, Carried, GithubBackend, Landing, Merge, MergeError,
};
use mergegate_core::store::{Shared, StoreError};
use store_sqlite::Store;

use crate::reply::Failure;
use crate::server::{AppState, Steps};
use crate::{branches, items, projects, reviews};

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
    AppState::detached(async move {
        let merge = state
            .stepwise(move |store, git, github| {
                let github = GithubBackend::new(git, github);
                // The hub's git is the local backend.
                let merge = merge::approve(store, git, &[git, &github], request);
                dropped(store, git, merge)
            })
            .await;
        changed(&state, &id);
        let merge = merge.map_err(failure)?;
        let stacked: Vec<_> = merge.stacked.iter().map(|i| i.id.clone()).collect();
        stacked_changed(&state, &stacked);
        Ok(Json(to_api(merge)))
    })
    .await
}

async fn retry(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<api::merge::Merge>, Failure> {
    let Path(id) = id?;
    let id = parse(&id)?;
    let merging = id.clone();
    AppState::detached(async move {
        let merge = state
            .stepwise(move |store, git, github| {
                let github = GithubBackend::new(git, github);
                let merge = merge::retry(store, git, &[git, &github], &merging);
                dropped(store, git, merge)
            })
            .await;
        changed(&state, &id);
        let merge = merge.map_err(failure)?;
        let stacked: Vec<_> = merge.stacked.iter().map(|i| i.id.clone()).collect();
        stacked_changed(&state, &stacked);
        Ok(Json(to_api(merge)))
    })
    .await
}

async fn cancel(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<api::item::Item>, Failure> {
    let Path(id) = id?;
    let id = parse(&id)?;
    // Not while the merge is landing: it waits until that's done.
    let item = state
        .stepwise(move |steps, _, _| steps.with(|store| merge::cancel(store, &id)))
        .await
        .map_err(failure)?;
    changed(&state, &item.id);
    Ok(Json(items::to_api(&item)))
}

/// What carrying an approval over came to.
pub(crate) struct Carry {
    /// For its agent: what came of it.
    pub warning: String,
    /// The items stacked on it that merged right after it.
    pub stacked: Vec<ItemId>,
}

/// Lands a new revision of `id` on the operator's standing approval when
/// it changes nothing since the approved one. Under [`AppState::landing`].
pub(crate) fn carry(
    store: &Steps<'_>,
    git: &HubGit,
    github: &GithubClient,
    id: &ItemId,
) -> Option<Carry> {
    let github = GithubBackend::new(git, github);
    let (from, merge) = match merge::carry(store, git, &[git, &github], id) {
        Ok(None) => return None,
        Ok(Some(Carried { from, merge })) => (from, Ok(merge)),
        Err(e) => (0, Err(e)),
    };
    let (landing, stacked) = match dropped(store, git, merge) {
        Ok(merge) => (merge.landing, merge.stacked),
        Err(e) => {
            return Some(Carry {
                warning: format!("the earlier approval couldn't carry over: {e}"),
                stacked: Vec::new(),
            });
        }
    };
    let stacked = stacked.into_iter().map(|item| item.id).collect();
    let outcome = match landing {
        Landing::Merged { commit } => format!("merged as {commit}"),
        Landing::Held { reason } => format!("the merge is held: {reason}"),
        Landing::Conflict { paths, .. } => {
            format!("the merge conflicts in {}", paths.join(", "))
        }
        Landing::Waiting { parent } => format!("it merges right after {parent}"),
    };
    Some(Carry {
        warning: format!(
            "the diff is unchanged since the operator approved r{from}, so the approval stands: {outcome}"
        ),
        stacked,
    })
}

/// Drops the branches of a merged item and of what merged right after it
/// from staging.
fn dropped(
    store: &Steps<'_>,
    git: &HubGit,
    merge: Result<Merge, MergeError>,
) -> Result<Merge, MergeError> {
    if let Ok(Merge {
        item,
        landing: Landing::Merged { .. },
        stacked,
    }) = &merge
    {
        store.with(|store: &mut Store| {
            branches::drop(store, git, &item.id);
            for child in stacked.iter().filter(|c| c.state == ItemState::Merged) {
                branches::drop(store, git, &child.id);
            }
        });
    }
    merge
}

/// Every outcome logs an event for the agent and moves the item. So can a
/// failure: one after the approval was recorded must still reach them, and
/// a wake with nothing new only costs a re-read.
pub(crate) fn changed(state: &AppState, id: &ItemId) {
    state.events.logged();
    // A merge closes the item, which starts its attachments' time to live.
    state.prune.notify_one();
    // And moves the target under every other open item.
    state.conflicts.notify_one();
    state.events.send(UiEvent::InboxChanged);
    state.events.send(UiEvent::ItemChanged {
        item: id.to_string(),
    });
}

/// Sends the items stacked on `id` changed: a merge or a new revision of
/// theirs logs an event for each, and may have merged them.
pub(crate) fn stacked_changed(state: &AppState, stacked: &[ItemId]) {
    if stacked.is_empty() {
        return;
    }
    state.events.logged();
    for id in stacked {
        state.events.send(UiEvent::ItemChanged {
            item: id.to_string(),
        });
    }
}

/// Why `item`'s merge is stuck, for the inbox and the review screen;
/// `landing` is [`AppState::landing_now`], asked after reading `item`.
pub(crate) fn blocked(
    store: &(impl FeedbackStore + ItemStore),
    item: &Item,
    landing: bool,
) -> Result<Option<api::merge::Blocked>, StoreError> {
    merge::blocked(store, item, landing)?
        .map(|blocked| -> Result<_, StoreError> {
            Ok(match blocked {
                Blocked::Held { reason } => api::merge::Blocked::Held { reason },
                Blocked::Conflict {
                    target_commit,
                    paths,
                } => api::merge::Blocked::Conflict {
                    target_commit,
                    paths,
                },
                // The store keeps the parent by ID; the operator knows it by name.
                Blocked::Waiting { parent } => api::merge::Blocked::Waiting {
                    parent: items::name_of(store, &parent)?,
                },
                Blocked::Behind { parent } => api::merge::Blocked::Behind {
                    parent: items::name_of(store, &parent)?,
                },
            })
        })
        .transpose()
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
        Landing::Waiting { parent } => api::merge::Outcome::Waiting {
            parent: parent.to_string(),
        },
    };
    api::merge::Merge {
        item: items::to_api(&merge.item),
        outcome,
        stacked: merge.stacked.iter().map(items::to_api).collect(),
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
        E::Review(e) => return reviews::failure(e),
        e => e,
    };
    let (status, code, hint) = match &e {
        E::Project(_) | E::Item(_) | E::Store(_) | E::Review(_) => unreachable!("answered above"),
        E::NoItem(_) => (S::NOT_FOUND, "item_not_found", Some("mg inbox".to_owned())),
        E::Closed(_) => (S::CONFLICT, "item_closed", None),
        E::Merging(_) => (S::CONFLICT, "item_merging", None),
        E::NotMerging(_) => (S::CONFLICT, "not_merging", None),
        E::NoRevision { .. } => (S::NOT_FOUND, "revision_not_found", None),
        E::Stale { .. } => (S::CONFLICT, "stale_revision", None),
        E::ParentAbandoned { .. } => (S::CONFLICT, "parent_abandoned", None),
        E::NoBackend { .. } => (S::CONFLICT, "backend_unavailable", None),
    };
    Failure::new(status, code, e.to_string(), hint)
}
