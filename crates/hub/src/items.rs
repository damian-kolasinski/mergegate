//! `/api/items` and `/api/inbox`: registering pushed work and the review
//! queue.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use mergegate_core::attachment::AttachmentStore;
use mergegate_core::item::{self, Item, ItemBackend, ItemError, ItemState, Submit};
use mergegate_core::project::{ProjectName, ProjectStore};
use mergegate_core::store::StoreError;

use crate::reply::Failure;
use crate::server::AppState;
use crate::{merge, projects, rules};

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
        guide: request.guide,
    };
    let app = state.clone();
    let (moved, submitted) = state
        .blocking_github(move |store, git, github| {
            // So a rule the operator just wrote applies to this revision
            // without waiting for the next poll.
            let moved = match ProjectName::parse(&request.project) {
                Ok(project) if store.project(&project)?.is_some() => {
                    app.rules.sync(store, &project)?
                }
                _ => rules::Changes::default(),
            };
            // The moves stand even when the submit fails, so the UI hears
            // of them either way.
            let submitted = item::submit(store, git, github, request);
            Ok::<_, ItemError>((moved, submitted))
        })
        .await
        .map_err(failure)?;
    rules::changed(&state, &moved);
    let submitted = submitted.map_err(failure)?;
    // A new wake command may be due for events already logged.
    state.wakes.recheck();
    if submitted.new_revision {
        state.highlight.notify_one();
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
            warnings: submitted.warnings,
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
                    .map(|item| {
                        let blocked = merge::blocked(store, item)?;
                        let demo = !store.attachments(&item.id)?.is_empty();
                        Ok(inbox_entry(item, blocked, demo))
                    })
                    .collect()
            };
            let mut pending = Vec::new();
            for project in store.projects()? {
                let changes = rules::waiting(store, &project.name)?;
                if changes > 0 {
                    pending.push(api::rules::PendingRules {
                        project: project.name.to_string(),
                        changes,
                    });
                }
            }
            Ok(api::item::Inbox {
                needs_you: entries(inbox.needs_you)?,
                with_agents: entries(inbox.with_agents)?,
                merging: entries(inbox.merging)?,
                rules: pending,
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
        backend: projects::backend_to_api(item.backend.kind()),
        target: item.target.clone(),
        revision: api::item::Revision {
            number: revision.number,
            head: revision.head.clone(),
            base: revision.base.clone(),
            files: revision.stat.files,
            additions: revision.stat.additions,
            deletions: revision.stat.deletions,
            review_files: revision.stat.review_files,
            submitted_at: revision.submitted_at.clone(),
        },
        link: link(&item.backend),
        created_at: item.created_at.clone(),
    }
}

fn link(backend: &ItemBackend) -> Option<api::item::Link> {
    match backend {
        ItemBackend::Github { pull, .. } => Some(api::item::Link {
            label: pull.to_string(),
            title: "Pull request on GitHub".into(),
            url: pull.url.clone(),
        }),
        ItemBackend::Local => None,
    }
}

fn inbox_entry(
    item: &Item,
    blocked: Option<api::merge::Blocked>,
    demo: bool,
) -> api::item::InboxItem {
    let revision = &item.revision;
    api::item::InboxItem {
        id: item.id.to_string(),
        title: item.title.clone(),
        branch: item.branch.clone(),
        revision: revision.number,
        files: revision.stat.files,
        additions: revision.stat.additions,
        deletions: revision.stat.deletions,
        review_files: revision.stat.review_files,
        created_at: item.created_at.clone(),
        updated_at: revision.submitted_at.clone(),
        blocked,
        link: link(&item.backend),
        demo,
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
