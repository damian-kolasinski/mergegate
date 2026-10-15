//! `GET /api/items/{id}/feedback` and `POST .../feedback/ack`: an item's
//! events for its agent, read by `mg wait` (long-polling) and `mg feedback`.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use mergegate_core::feedback::{
    self, Event, EventKind, Feedback, FeedbackError, NoticeKind, Source, Wake,
};
use mergegate_core::item::{Item, ItemId};
use serde::Deserialize;
use store_sqlite::Store;
use tokio::time::Instant;

use crate::reply::Failure;
use crate::server::AppState;
use crate::{comments, external, items, replies, reviews};

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
    let mut last = query.wait == 0;
    // Its agent is awake while this reads, and needs no wake command run.
    let _reading = state.wakes.reading(&id);
    loop {
        log.mark_unchanged();
        let read = {
            let id = id.clone();
            state
                .blocking(move |store, _| feedback::feedback(store, &id, query.since))
                .await
                .map_err(failure)?
        };
        let woke = wake.by(&read.events) || !read.item.state.is_open();
        if last || woke {
            // A wait's answer at its deadline can hold events its `mg wait`
            // isn't waiting for, and skips.
            if !(query.wait == 0 || woke) || read.events.is_empty() {
                return Ok(Json(to_api(&id, read)));
            }
            let (read, released) = {
                let id = id.clone();
                state
                    .blocking(move |store, _| handing(store, &id, query.since, read))
                    .await
                    .map_err(failure)?
            };
            if released {
                external::released(&state, &id);
            }
            state.wakes.handed(&id, read.cursor);
            return Ok(Json(to_api(&id, read)));
        }
        // The store stays unlocked while this waits.
        tokio::select! {
            changed = log.changed() => last = changed.is_err(),
            () = tokio::time::sleep_until(deadline) => last = true,
            _ = stopping.wait_for(|stopping| *stopping) => last = true,
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
        .blocking(move |store, _| feedback::acknowledge(store, &id, request.cursor))
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

fn to_api(id: &ItemId, read: Feedback) -> api::feedback::Feedback {
    api::feedback::Feedback {
        item: id.to_string(),
        state: items::state_to_api(read.item.state),
        events: read
            .events
            .into_iter()
            .map(|e| event_to_api(id, &read.item, e))
            .collect(),
        cursor: read.cursor,
    }
}

pub(crate) fn event_to_api(id: &ItemId, item: &Item, event: Event) -> api::feedback::Event {
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
            NoticeKind::Abandoned { reason } => api::feedback::EventBody::ItemAbandoned { reason },
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
            NoticeKind::Merged { commit } => api::feedback::EventBody::ItemMerged {
                commit,
                branch: item.branch.clone(),
            },
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
