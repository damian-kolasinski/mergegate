//! `POST /api/items/{id}/review-time`: the operator's active time on an
//! item's review screen, which the UI reports as it goes, and the stats
//! that `item.merged` and `item.abandoned` carry for agents' telemetry.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use mergegate_core::feedback::{Event, EventKind, NoticeKind};
use mergegate_core::item::{Item, ItemId, ItemStore};
use mergegate_core::review_time::{self, ReviewTimeError, ReviewTimeStore, Spent, Stats};
use mergegate_core::store::StoreError;

use crate::reply::Failure;
use crate::reviews::verdict_to_api;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/items/{id}/review-time", post(record))
}

async fn record(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
    request: Result<Json<api::review_time::ReviewTime>, JsonRejection>,
) -> Result<StatusCode, Failure> {
    let Path(id) = id?;
    let Json(request) = request?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let spent = Spent {
        id,
        revision: request.revision,
        ms: request.ms,
    };
    state
        .blocking(move |store, _| review_time::record(store, spent))
        .await
        .map_err(failure)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The stats of the items that `events` close, for [`crate::feedback::event_to_api`].
pub(crate) fn of_closes<'a, S: ItemStore + ReviewTimeStore>(
    store: &S,
    events: impl IntoIterator<Item = (&'a Item, &'a Event)>,
) -> Result<HashMap<ItemId, api::review_time::ReviewStats>, StoreError> {
    let mut stats = HashMap::new();
    for (item, event) in events {
        if let EventKind::Notice(notice) = &event.kind
            && matches!(
                notice.kind,
                NoticeKind::Merged { .. } | NoticeKind::Abandoned { .. }
            )
            && !stats.contains_key(&item.id)
            && let Some(of_item) = review_time::stats(store, item)?
        {
            stats.insert(item.id.clone(), to_api(of_item));
        }
    }
    Ok(stats)
}

fn to_api(stats: Stats) -> api::review_time::ReviewStats {
    api::review_time::ReviewStats {
        opened_at: stats.opened_at,
        closed_at: stats.closed_at,
        review_ms: stats.review_ms,
        turns: stats
            .turns
            .into_iter()
            .map(|t| api::review_time::TurnStats {
                turn: t.number,
                revision: t.revision,
                verdict: t.verdict.map(verdict_to_api),
                sent_at: t.sent_at,
                review_ms: t.review_ms,
            })
            .collect(),
        revisions: stats
            .revisions
            .into_iter()
            .map(|r| api::review_time::RevisionStats {
                revision: r.number,
                submitted_at: r.submitted_at,
                review_ms: r.review_ms,
            })
            .collect(),
    }
}

fn failure(e: ReviewTimeError) -> Failure {
    use ReviewTimeError as E;
    use StatusCode as S;
    let (status, code, hint) = match &e {
        E::NoItem(_) => (S::NOT_FOUND, "item_not_found", Some("mg inbox".to_owned())),
        E::Closed(_) => (S::CONFLICT, "item_closed", None),
        E::Merging(_) => (S::CONFLICT, "item_merging", None),
        E::NoRevision { .. } => (S::NOT_FOUND, "revision_not_found", None),
        E::TooLong { .. } => (S::BAD_REQUEST, "invalid_review_time", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}
