//! `/api/items` and `/api/inbox`: registering pushed work and the review
//! queue.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use mergegate_core::github::Pull;
use mergegate_core::item::{self, Item, ItemError, ItemState, Submit};
use mergegate_core::store::StoreError;

use crate::reply::Failure;
use crate::server::AppState;
use crate::{merge, projects};

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
        worktree: request.worktree,
        wake: request.wake,
    };
    let submitted = state
        .blocking_github(move |store, git, github| item::submit(store, git, github, request))
        .await
        .map_err(failure)?;
    // A new wake command may be due for events already logged.
    state.wakes.recheck();
    if submitted.new_revision {
        state.events.send(UiEvent::InboxChanged);
        state.events.send(UiEvent::ItemChanged {
            item: submitted.item.id.to_string(),
        });
    }
    let opened = submitted.new_revision && submitted.item.revision.number == 1;
    let status = if opened {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((
        status,
        Json(api::item::Submitted {
            item: to_api(&submitted.item),
            new_revision: submitted.new_revision,
        }),
    ))
}

async fn inbox(State(state): State<Arc<AppState>>) -> Result<Json<api::item::Inbox>, Failure> {
    let inbox = state
        .blocking(|store, _| {
            let inbox = item::inbox(store)?;
            let entries = |items: Vec<Item>| -> Result<Vec<_>, StoreError> {
                items
                    .iter()
                    .map(|item| Ok(inbox_entry(item, merge::blocked(store, item)?)))
                    .collect()
            };
            Ok(api::item::Inbox {
                needs_you: entries(inbox.needs_you)?,
                with_agents: entries(inbox.with_agents)?,
                merging: entries(inbox.merging)?,
            })
        })
        .await
        .map_err(|e: StoreError| failure(e.into()))?;
    Ok(Json(inbox))
}

pub(crate) fn to_api(item: &Item) -> api::item::Item {
    let revision = &item.revision;
    api::item::Item {
        id: item.id.to_string(),
        title: item.title.clone(),
        branch: item.branch.clone(),
        state: state_to_api(item.state),
        backend: projects::backend_to_api(item.backend),
        target: item.target.clone(),
        revision: api::item::Revision {
            number: revision.number,
            head: revision.head.clone(),
            base: revision.base.clone(),
            files: revision.stat.files,
            additions: revision.stat.additions,
            deletions: revision.stat.deletions,
            submitted_at: revision.submitted_at.clone(),
        },
        pull: item.pull.as_ref().map(pull_to_api),
        created_at: item.created_at.clone(),
    }
}

fn pull_to_api(pull: &Pull) -> api::item::Pull {
    api::item::Pull {
        number: pull.number,
        url: pull.url.clone(),
    }
}

fn inbox_entry(item: &Item, blocked: Option<api::merge::Blocked>) -> api::item::InboxItem {
    let revision = &item.revision;
    api::item::InboxItem {
        id: item.id.to_string(),
        title: item.title.clone(),
        branch: item.branch.clone(),
        revision: revision.number,
        files: revision.stat.files,
        additions: revision.stat.additions,
        deletions: revision.stat.deletions,
        created_at: item.created_at.clone(),
        updated_at: revision.submitted_at.clone(),
        blocked,
        pull: item.pull.as_ref().map(pull_to_api),
    }
}

pub(crate) fn state_to_api(state: ItemState) -> api::item::ItemState {
    match state {
        ItemState::NeedsReview => api::item::ItemState::NeedsReview,
        ItemState::ChangesRequested => api::item::ItemState::ChangesRequested,
        ItemState::Merging => api::item::ItemState::Merging,
        ItemState::Merged => api::item::ItemState::Merged,
        ItemState::Abandoned => api::item::ItemState::Abandoned,
    }
}

pub(crate) fn failure(e: ItemError) -> Failure {
    use ItemError as E;
    use StatusCode as S;
    let e = match e {
        E::Project(e) => return projects::failure(e),
        E::Github(e) => return projects::github_failure(e),
        e => e,
    };
    let (status, code, hint) = match &e {
        E::Project(_) | E::Github(_) => unreachable!("answered above"),
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
        E::NoItem(_) => (S::NOT_FOUND, "item_not_found", Some("mg inbox".to_owned())),
        E::Closed(_) => (S::CONFLICT, "item_closed", None),
        E::NothingToSubmit { .. } => (S::BAD_REQUEST, "nothing_to_submit", None),
        // Pushing over it would drop commits the hub didn't push.
        E::BranchMoved { .. } => (S::CONFLICT, "branch_moved", None),
        // Its pull request was closed on GitHub, which can't reopen it.
        E::BranchGone { .. } => (S::CONFLICT, "branch_gone", None),
        E::Git(_) => (S::INTERNAL_SERVER_ERROR, "git_failed", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}
