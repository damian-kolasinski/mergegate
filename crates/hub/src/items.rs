//! `/api/items` and `/api/inbox`: registering pushed work and the review
//! queue.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use mergegate_core::attachment::AttachmentStore;
use mergegate_core::interdiff::{self, ChangedSince};
use mergegate_core::item::{
    self, Item, ItemBackend, ItemError, ItemId, ItemState, ItemStore, On, StackError, Submit,
};
use mergegate_core::project::{ProjectName, ProjectStore};
use mergegate_core::review::Waiting;
use mergegate_core::store::StoreError;

use crate::guard::Caller;
use crate::reply::Failure;
use crate::server::AppState;
use crate::{merge, priority, projects, rules};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/items", post(submit))
        .route("/api/inbox", get(inbox))
}

async fn submit(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<Caller>,
    request: Result<Json<api::item::Submit>, JsonRejection>,
) -> Result<(StatusCode, Json<api::item::Submitted>), Failure> {
    let Json(mut request) = request?;
    // A wake command runs on the hub's machine, in the submitting worktree,
    // so only an agent on that machine may leave one.
    if let Caller::Device(device, ..) = &caller {
        if request.wake.is_some() {
            return Err(Failure::new(
                StatusCode::FORBIDDEN,
                "wake_remote",
                format!(
                    "{} submits over the tailnet, and a wake command runs on the hub's machine",
                    device.name
                ),
                Some("submit without --wake and run: mg wait".into()),
            ));
        }
        // The item may have moved here from a local agent; its wake would
        // run in a worktree this agent isn't in.
        request.wake = Some(String::new());
    }
    let on = match (&request.on, ProjectName::parse(&request.project)) {
        (Some(on), Ok(project)) => Some(On::parse(on, &project).map_err(|e| {
            Failure::new(
                StatusCode::BAD_REQUEST,
                "invalid_item",
                format!("--on {on}: {e}"),
                Some("name an open item, as app#41 or 41, or none".into()),
            )
        })?),
        _ => None,
    };
    let request = Submit {
        project: request.project,
        branch: request.branch,
        head: request.head,
        title: request.title,
        worktree: request.worktree,
        wake: request.wake,
        guide: request.guide,
        priority: request.priority.map(priority::from_api),
        note: request.note,
        on,
    };
    let app = state.clone();
    let (moved, submitted, carried, (stacked, parent)) = state
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
            let mut submitted = item::submit(store, git, github, request);
            let mut carried = false;
            let mut stacked = Vec::new();
            // The operator approved the diff, not a title it would land under.
            if let Ok(done) = &mut submitted
                && done.new_revision
                && !done.retitled
                && let Some(carry) = merge::carry(store, git, github, &done.item.id)
            {
                carried = true;
                done.warnings.push(carry.warning);
                stacked = carry.stacked;
                // The merge happened: failing to re-read it mustn't fail the
                // submit, whose item then shows the state before it.
                if let Ok(Some(item)) = store.item(&done.item.id) {
                    done.item = item;
                }
            }
            // A new revision told the item stacked on it to rebase. The
            // submit is recorded, so failing to read that mustn't fail it.
            if let Ok(done) = &submitted
                && done.new_revision
                && let Ok(Some(child)) = store.child(&done.item.id)
            {
                stacked.push(child.id);
            }
            // Its parent's stack trail now lists it.
            if let Ok(done) = &submitted
                && let Some(parent) = &done.item.parent
            {
                stacked.push(parent.clone());
            }
            // The submit is recorded, so a failed read leaves the parent's ID.
            let parent = submitted
                .as_ref()
                .ok()
                .and_then(|done| done.item.parent.as_ref())
                .map(|parent| name_of(store, parent).unwrap_or_else(|_| parent.to_string()));
            Ok::<_, ItemError>((moved, submitted, carried, (stacked, parent)))
        })
        .await
        .map_err(failure)?;
    rules::changed(&state, &moved);
    let submitted = submitted.map_err(failure)?;
    if carried {
        merge::changed(&state, &submitted.item.id);
    }
    merge::stacked_changed(&state, &stacked);
    // A new wake command may be due for events already logged.
    state.wakes.recheck();
    if submitted.new_revision {
        state.highlight.notify_one();
        // Submitting fetched the target, which may have moved.
        state.conflicts.notify_one();
        state.judge.notify_one();
    }
    // Even at the same head, the title or guide may have changed.
    state.events.send(UiEvent::InboxChanged);
    state.events.send(UiEvent::ItemChanged {
        item: submitted.item.id.to_string(),
    });
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
            parent,
        }),
    ))
}

async fn inbox(State(state): State<Arc<AppState>>) -> Result<Json<api::item::Inbox>, Failure> {
    let inbox = state
        .blocking(|store, _| {
            let inbox = item::inbox(store)?;
            let entries = |items: &[Item]| -> Result<Vec<_>, StoreError> {
                items
                    .iter()
                    .map(|item| {
                        let blocked = merge::blocked(store, item)?;
                        let demo = !store.attachments(&item.id)?.is_empty();
                        let changed = interdiff::changed_since_review(store, item)?;
                        let waiting = inbox.waiting.get(&item.id).copied().unwrap_or_default();
                        let behind = item.state.is_open() && item::behind(store, item)?;
                        let mut entry = inbox_entry(item, blocked, demo, changed, waiting);
                        entry.parent = item
                            .parent
                            .as_ref()
                            .map(|p| name_of(store, p))
                            .transpose()?;
                        entry.behind = behind;
                        Ok(entry)
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
                needs_you: entries(&inbox.needs_you)?,
                with_agents: entries(&inbox.with_agents)?,
                merging: entries(&inbox.merging)?,
                closed: entries(&inbox.closed)?,
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
        name: item.name().to_string(),
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
        priority: item.priority.map(priority::to_api),
        note: item.note.clone(),
        parent: item.parent.as_ref().map(ToString::to_string),
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
    changed: Option<ChangedSince>,
    waiting: Waiting,
) -> api::item::InboxItem {
    let revision = &item.revision;
    api::item::InboxItem {
        id: item.id.to_string(),
        name: item.name().to_string(),
        title: item.title.clone(),
        branch: item.branch.clone(),
        state: state_to_api(item.state),
        target: item.target.clone(),
        revision: revision.number,
        files: revision.stat.files,
        additions: revision.stat.additions,
        deletions: revision.stat.deletions,
        review_files: revision.stat.review_files,
        created_at: item.created_at.clone(),
        updated_at: item.updated_at().to_owned(),
        blocked,
        link: link(&item.backend),
        demo,
        changed_since: changed.map(changed_to_api),
        priority: item.priority.map(priority::to_api),
        note: item.note.clone(),
        questions: waiting.questions,
        rejections: waiting.rejections,
        parent: None,
        behind: false,
        votes: waiting.votes,
    }
}

/// The name of the item `id` is of, or `id` for one the hub doesn't have.
pub(crate) fn name_of(store: &impl ItemStore, id: &ItemId) -> Result<String, StoreError> {
    let item = store.item(id)?;
    Ok(item
        .map_or_else(|| id.clone(), |item| item.name())
        .to_string())
}

pub(crate) fn changed_to_api(changed: ChangedSince) -> api::item::ChangedSince {
    api::item::ChangedSince {
        revision: changed.revision,
        files: changed.files,
        approved: changed.approved,
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
        E::Stack(e) => stack_failure(e),
        E::Git(_) => (S::INTERNAL_SERVER_ERROR, "git_failed", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}

fn stack_failure(e: &StackError) -> (StatusCode, &'static str, Option<String>) {
    use StackError as E;
    use StatusCode as S;
    let none = Some("mg submit --on none".to_owned());
    match e {
        E::OtherProject { .. } => (S::BAD_REQUEST, "other_project", None),
        E::ParentClosed(_) => (S::CONFLICT, "parent_closed", none),
        E::OnItself(_) => (S::BAD_REQUEST, "on_itself", None),
        E::Cycle { .. } => (S::CONFLICT, "stack_cycle", None),
        E::HasChild { child, .. } => (
            S::CONFLICT,
            "has_child",
            Some(format!("mg submit --on {child}")),
        ),
        E::OtherTarget { .. } => (S::CONFLICT, "other_target", None),
        E::OtherBackend { .. } => (S::CONFLICT, "other_backend", None),
        E::NotOnParent { parent_branch, .. } => (
            S::CONFLICT,
            "not_on_parent",
            Some(format!("git rebase mergegate/{parent_branch}")),
        ),
        E::ParentMerged {
            target, upstream, ..
        } => (
            S::CONFLICT,
            "parent_merged",
            Some(format!("git rebase --onto mergegate/{target} {upstream}")),
        ),
    }
}
