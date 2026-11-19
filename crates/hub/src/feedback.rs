//! `GET /api/items/{id}/feedback` and `POST .../feedback/ack`: an item's
//! events for its agent, read by `mg wait` (long-polling) and `mg feedback`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use mergegate_core::feedback::{
    self, Event, EventKind, Feedback, FeedbackError, NoticeKind, Source, Wake,
};
use mergegate_core::item::{Item, ItemId, ItemStore};
use mergegate_core::store::StoreError;
use serde::Deserialize;
use store_sqlite::Store;
use tokio::time::Instant;

use crate::guard::{self, Caller};
use crate::reply::Failure;
use crate::server::AppState;
use crate::{comments, external, items, presence, replies, review_time, reviews};

/// The longest one request waits; `mg wait` asks again to wait longer.
const MAX_WAIT: Duration = Duration::from_secs(3600);

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/items/{id}/feedback", get(read))
        .route("/api/items/{id}/feedback/ack", post(acknowledge))
}

#[derive(Deserialize)]
struct ReadQuery {
    since: Option<u64>,
    /// Seconds.
    #[serde(default)]
    wait: u64,
    #[serde(default, rename = "for")]
    wake: api::feedback::Wake,
}

async fn read(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<Caller>,
    id: Result<Path<String>, PathRejection>,
    query: Result<Query<ReadQuery>, QueryRejection>,
) -> Result<Json<api::feedback::Feedback>, Failure> {
    let Path(id) = id?;
    let Query(query) = query?;
    let id = parse(&id)?;
    let wake = match query.wake {
        api::feedback::Wake::Any => Wake::Any,
        api::feedback::Wake::Operator => Wake::Operator,
    };
    let deadline = Instant::now() + Duration::from_secs(query.wait).min(MAX_WAIT);
    let mut log = state.events.log();
    let mut stopping = state.events.stopping();
    let mut admissions = caller.admissions(&state);
    let mut last = query.wait == 0;
    // Its agent is awake while this reads, and needs no wake command run.
    let _reading = state.wakes.reading(&id);
    let _present = presence::reading(&state, &id);
    let mut since = query.since;
    loop {
        log.mark_unchanged();
        let (read, related) = {
            let id = id.clone();
            state
                .blocking(move |store, _| {
                    let read = feedback::feedback(store, &id, since)?;
                    let related = relate(store, read.events.iter().map(|e| (&read.item, e)))?;
                    Ok::<_, FeedbackError>((read, related))
                })
                .await
                .map_err(failure)?
        };
        // A `since` past the item's events reads from its newest; staying
        // there keeps an event logged meanwhile, below `since`, in the next.
        if since.is_some() && read.events.is_empty() {
            since = Some(read.cursor);
        }
        let woke = wake.by(&read.events) || !read.item.state.is_open();
        if last || woke {
            // A wait's answer at its deadline can hold events its `mg wait`
            // isn't waiting for, and skips.
            if !(query.wait == 0 || woke) || read.events.is_empty() {
                return Ok(Json(to_api(&id, read, &related)));
            }
            let ((read, released), related) = {
                let id = id.clone();
                state
                    .blocking(move |store, _| {
                        let handed = handing(store, &id, since, read)?;
                        let (read, _) = &handed;
                        let related = relate(store, read.events.iter().map(|e| (&read.item, e)))?;
                        Ok::<_, FeedbackError>((handed, related))
                    })
                    .await
                    .map_err(failure)?
            };
            if released {
                external::released(&state, &id);
            }
            state.wakes.handed(&id, read.cursor);
            return Ok(Json(to_api(&id, read, &related)));
        }
        // The store stays unlocked while this waits.
        let mut readmit = false;
        tokio::select! {
            changed = log.changed() => last = changed.is_err(),
            () = tokio::time::sleep_until(deadline) => last = true,
            _ = stopping.wait_for(|stopping| *stopping) => last = true,
            () = admissions.changed() => readmit = true,
        }
        // A device denied while it waits gets the refusal, not events.
        if readmit {
            guard::recheck(&state, &caller).await?;
        }
    }
}

async fn acknowledge(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
    request: Result<Json<api::feedback::Acknowledge>, JsonRejection>,
) -> Result<Json<api::feedback::Acknowledged>, Failure> {
    let Path(id) = id?;
    let Json(request) = request?;
    let id = parse(&id)?;
    let delivered = state
        .blocking(move |store, _| feedback::acknowledge(store, &id, request.cursor, request.since))
        .await
        .map_err(failure)?;
    Ok(Json(api::feedback::Acknowledged { delivered }))
}

/// `read` as it goes to the agent, with any held comments it takes along
/// (`release_on_read`); whether it took some.
fn handing(
    store: &mut Store,
    id: &ItemId,
    since: Option<u64>,
    read: Feedback,
) -> Result<(Feedback, bool), FeedbackError> {
    match mergegate_core::external::release_on_read(store, id, since, &read.events) {
        Ok(true) => Ok((feedback::feedback(store, id, since)?, true)),
        Ok(false) => Ok((read, false)),
        // The comments wait for the poller, and the read goes ahead.
        Err(e) => {
            tracing::error!("handing {id}'s held comments to its agent: {e}");
            Ok((read, false))
        }
    }
}

fn parse(id: &str) -> Result<ItemId, Failure> {
    ItemId::parse(id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))
}

fn to_api(id: &ItemId, read: Feedback, related: &Related) -> api::feedback::Feedback {
    api::feedback::Feedback {
        item: id.to_string(),
        name: read.item.name().to_string(),
        state: items::state_to_api(read.item.state),
        events: read
            .events
            .into_iter()
            .map(|e| event_to_api(id, &read.item, e, related))
            .collect(),
        cursor: read.cursor,
    }
}

/// What events say beyond their own rows, which [`event_to_api`] reads.
pub(crate) struct Related {
    /// The names of the parents they speak of, which the store keeps by ID
    /// and agents know by name.
    parents: HashMap<ItemId, String>,
    /// The stats of the items they close.
    stats: HashMap<ItemId, api::review_time::ReviewStats>,
}

/// What `events`, each with its item, need of [`Related`].
pub(crate) fn relate<'a>(
    store: &Store,
    events: impl IntoIterator<Item = (&'a Item, &'a Event)>,
) -> Result<Related, StoreError> {
    let events: Vec<_> = events.into_iter().collect();
    Ok(Related {
        parents: parent_names(store, events.iter().map(|(_, e)| *e))?,
        stats: review_time::of_closes(store, events)?,
    })
}

fn parent_names<'a>(
    store: &impl ItemStore,
    events: impl IntoIterator<Item = &'a Event>,
) -> Result<HashMap<ItemId, String>, StoreError> {
    let mut names = HashMap::new();
    for event in events {
        if let EventKind::Notice(notice) = &event.kind
            && let NoticeKind::ParentUpdated { parent, .. }
            | NoticeKind::ParentAbandoned { parent, .. } = &notice.kind
            && !names.contains_key(parent)
        {
            names.insert(parent.clone(), items::name_of(store, parent)?);
        }
    }
    Ok(names)
}

/// `related` is what `event` speaks of, as [`relate`] found it.
pub(crate) fn event_to_api(
    id: &ItemId,
    item: &Item,
    event: Event,
    related: &Related,
) -> api::feedback::Event {
    let name = |parent: &ItemId| {
        related
            .parents
            .get(parent)
            .cloned()
            .unwrap_or_else(|| parent.to_string())
    };
    let target = &item.target;
    let revision = event.revision();
    let source = source_to_api(event.source());
    let body = match event.kind {
        EventKind::ReviewSubmitted(review) => api::feedback::EventBody::ReviewSubmitted {
            verdict: reviews::verdict_to_api(review.verdict),
            summary: review.summary,
            comments: review.comments.iter().map(comments::to_api).collect(),
            replies: review.replies.iter().map(replies::sent).collect(),
        },
        EventKind::Notice(notice) => match notice.kind {
            NoticeKind::Abandoned { reason } => api::feedback::EventBody::ItemAbandoned {
                reason,
                stats: related.stats.get(id).cloned(),
            },
            NoticeKind::MergeHeld { reason } => api::feedback::EventBody::MergeHeld { reason },
            NoticeKind::MergeCancelled => api::feedback::EventBody::MergeCancelled {},
            NoticeKind::MergeConflict {
                target_commit,
                paths,
            } => api::feedback::EventBody::MergeConflict {
                target_commit,
                paths,
                // Run in the agent's worktree, whose `mergegate` remote
                // `mg init` set up.
                commands: vec![
                    "git fetch mergegate".to_owned(),
                    format!("git rebase mergegate/{target}"),
                ],
            },
            // Detaching in place would leave the worktree on the pre-squash
            // commit, behind the target the item just landed on. The hub
            // deletes merged items' branches, which a plain fetch would keep
            // as stale `mergegate/*` refs.
            NoticeKind::Merged { commit } => api::feedback::EventBody::ItemMerged {
                commit,
                branch: item.branch.clone(),
                commands: vec![
                    "git fetch --prune mergegate".to_owned(),
                    format!("git switch --detach mergegate/{target}"),
                    format!("git branch -D {}", item.branch),
                ],
                stats: related.stats.get(id).cloned(),
            },
            // `--onto` keeps the parent's old commits out: once it merged,
            // they're on the target squashed into one, which a plain rebase
            // would try to apply again.
            NoticeKind::ParentUpdated {
                parent,
                merged,
                onto,
                upstream,
            } => api::feedback::EventBody::ParentUpdated {
                parent: name(&parent),
                merged,
                commands: vec![
                    "git fetch mergegate".to_owned(),
                    if merged {
                        format!("git rebase --onto mergegate/{target} {upstream}")
                    } else {
                        format!("git rebase --onto {onto} {upstream}")
                    },
                ],
                onto,
                upstream,
            },
            NoticeKind::ParentAbandoned { parent, upstream } => {
                api::feedback::EventBody::ParentAbandoned {
                    parent: name(&parent),
                    commands: vec![
                        "git fetch mergegate".to_owned(),
                        format!("git rebase --onto mergegate/{target} {upstream}"),
                    ],
                    upstream,
                }
            }
        },
        EventKind::ExternalComments(batch) => external::batch_to_api(&batch),
        EventKind::Override(o) => api::feedback::EventBody::CommentOverride {
            comment: Box::new(comments::to_api(&o.comment)),
            reply: replies::sent(&o.reply),
        },
    };
    api::feedback::Event {
        cursor: event.cursor,
        item: id.to_string(),
        name: item.name().to_string(),
        revision,
        source,
        body,
    }
}

pub(crate) fn source_to_api(source: Source) -> api::feedback::Source {
    match source {
        Source::Operator => api::feedback::Source::Operator,
        Source::Agent => api::feedback::Source::Agent,
        Source::External => api::feedback::Source::External,
    }
}

pub(crate) fn source_from_api(source: api::feedback::Source) -> Source {
    match source {
        api::feedback::Source::Operator => Source::Operator,
        api::feedback::Source::Agent => Source::Agent,
        api::feedback::Source::External => Source::External,
    }
}

pub(crate) fn failure(e: FeedbackError) -> Failure {
    use FeedbackError as E;
    use StatusCode as S;
    let (status, code, hint) = match &e {
        E::NoItem(_) => (S::NOT_FOUND, "item_not_found", Some("mg inbox".to_owned())),
        E::NoEvent { .. } => (S::BAD_REQUEST, "invalid_cursor", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}
