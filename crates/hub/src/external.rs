//! External review: comments on a github item's pull request. The poller
//! brings them in, and closes an item whose pull request was merged or
//! closed on GitHub; `POST /api/items/{id}/sync` does both at once, for when
//! polling missed something. `POST /api/comments/{comment}/vote` is the
//! operator's vote on a comment before the agent gets it, and
//! `.../agree` and `.../override` their say on the agent's rejection of one.

pub(crate) mod bots;
mod follow;
mod poll;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use api::event::UiEvent;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use mergegate_core::external::{self, Batch, ExternalError, Ingested, Vote};
use mergegate_core::feedback::{Event, EventKind};
use mergegate_core::github::PullCommentKind;
use mergegate_core::item::{ItemId, ItemStore};
use mergegate_core::review::{Comment, ReviewError, ReviewStore};

use crate::reply::Failure;
use crate::server::AppState;
use crate::{comments, projects, replies, reviews};

pub(crate) use poll::run;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/items/{id}/sync", post(sync))
        .route("/api/comments/{comment}/vote", post(vote))
        .route("/api/comments/{comment}/agree", post(agree))
        .route("/api/comments/{comment}/override", post(override_rejection))
}

async fn sync(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<api::review::Synced>, Failure> {
    let Path(id) = id?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let ingested = pull(&state, &id, true).await.map_err(failure)?;
    let released = {
        let id = id.clone();
        state
            .blocking(move |store, _| external::release(store, &id))
            .await
            .map_err(failure)?
    };
    changed(&state, &id, ingested, released.is_some());
    // The comments went either way; the next poll follows the pull request
    // again.
    if let Err(e) = follow::follow(&state, &id).await {
        tracing::warn!("following {id}'s pull request: {e}");
    }
    if let Err(e) = bots::watch(&state, &id).await {
        tracing::warn!("reading {id}'s reactions on GitHub: {e}");
    }
    Ok(Json(api::review::Synced {
        new: ingested.new as u32,
        edited: ingested.edited as u32,
        released: released.as_ref().map_or(0, carried) as u32,
    }))
}

async fn vote(
    State(state): State<Arc<AppState>>,
    comment: Result<Path<String>, PathRejection>,
    request: Result<Json<api::review::CastVote>, JsonRejection>,
) -> Result<Json<api::review::Comment>, Failure> {
    let Path(comment) = comment?;
    let Json(request) = request?;
    let comment = comments::comment_id(&comment)?;
    let vote = match request.vote {
        api::review::Vote::Up => Vote::Up,
        api::review::Vote::Down => Vote::Down,
    };
    let (voted, released) = state
        .blocking(move |store, _| {
            let item = external::vote(store, comment, vote, now())?.item;
            // The last vote the batch waited for sends it, rather than the
            // next poll.
            let released = external::release_due(store, &item, now(), poll::timing())?;
            let voted = store
                .comment(comment)?
                .ok_or(ExternalError::NoComment(comment))?;
            Ok::<_, ExternalError>((voted, released))
        })
        .await
        .map_err(failure)?;
    if released.is_some() {
        state.events.logged();
    }
    // The vote no longer waits on the operator in the inbox.
    state.events.send(UiEvent::ItemChanged {
        item: voted.item.to_string(),
    });
    state.events.send(UiEvent::InboxChanged);
    Ok(Json(comments::to_api(&voted)))
}

async fn agree(
    State(state): State<Arc<AppState>>,
    comment: Result<Path<String>, PathRejection>,
) -> Result<Json<api::review::Comment>, Failure> {
    let Path(comment) = comment?;
    let comment = comments::comment_id(&comment)?;
    let agreed = state
        .blocking(move |store, _| external::agree(store, comment))
        .await
        .map_err(failure)?;
    state.events.send(UiEvent::ItemChanged {
        item: agreed.item.to_string(),
    });
    // The rejection no longer waits on the operator in the inbox.
    state.events.send(UiEvent::InboxChanged);
    Ok(Json(comments::to_api(&agreed)))
}

async fn override_rejection(
    State(state): State<Arc<AppState>>,
    comment: Result<Path<String>, PathRejection>,
    request: Result<Json<api::review::OverrideRejection>, JsonRejection>,
) -> Result<(StatusCode, Json<api::review::Reply>), Failure> {
    let Path(comment) = comment?;
    let Json(request) = request?;
    let comment = comments::comment_id(&comment)?;
    let (reply, _) = state
        .blocking(move |store, _| external::override_rejection(store, comment, request.note))
        .await
        .map_err(failure)?;
    state.events.logged();
    state.events.send(UiEvent::ItemChanged {
        item: reply.item.to_string(),
    });
    state.events.send(UiEvent::InboxChanged);
    Ok((StatusCode::CREATED, Json(replies::sent(&reply))))
}

/// Brings `id`'s pull request comments in. GitHub is asked with the store
/// unlocked: a slow GitHub mustn't hold up every other request.
async fn pull(state: &Arc<AppState>, id: &ItemId, fresh: bool) -> Result<Ingested, ExternalError> {
    let _pulling = state.pulling.lock().await;
    let pull = {
        let id = id.clone();
        state
            .blocking(move |store, _| {
                let item = store
                    .item(&id)?
                    .ok_or_else(|| ReviewError::NoItem(id.clone()))?;
                let pull = external::pull_of(&item).map(|(repo, number)| (repo.clone(), number));
                let (repo, number) = pull.ok_or_else(|| ExternalError::NotGithub(id.clone()))?;
                Ok::<_, ExternalError>((repo, number, external::held_keys(store, &id)?))
            })
            .await?
    };
    let (repo, number, held) = pull;
    let (found, gone) = state
        .github(move |github| {
            let mut found = external::fetch(github, &repo, number, fresh)?;
            let gone = external::confirm_gone(github, &repo, number, &held, &mut found)?;
            Ok::<_, ExternalError>((found, gone))
        })
        .await?;
    let id = id.clone();
    state
        .blocking(move |store, git| {
            let mut ingested = external::ingest(store, git, &id, found, &gone, now())?;
            // What came in counts, and is told to the UI, though lapsing failed.
            match external::lapse(store, git, &id) {
                Ok(lapsed) => ingested.lapsed = lapsed,
                Err(e) => tracing::warn!("lapsing {id}'s outdated comments: {e}"),
            }
            Ok(ingested)
        })
        .await
}

/// Tells open UIs, and readers waiting on the log, what changed.
fn changed(state: &AppState, id: &ItemId, ingested: Ingested, released: bool) {
    if released {
        state.events.logged();
    }
    if ingested.changed() || released {
        state.events.send(UiEvent::ItemChanged {
            item: id.to_string(),
        });
        state.events.send(UiEvent::InboxChanged);
    }
}

/// For a release outside the poller.
pub(crate) fn released(state: &AppState, id: &ItemId) {
    changed(state, id, Ingested::default(), true);
}

/// Milliseconds since the epoch, the clock held comments are timed on.
pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

pub(crate) fn to_api(comment: &Comment) -> Option<api::review::ExternalComment> {
    let external = comment.external.as_ref()?;
    Some(api::review::ExternalComment {
        author: external.login.clone(),
        bot: external.bot,
        kind: match external.kind {
            PullCommentKind::ReviewComment => api::review::ExternalKind::ReviewComment,
            PullCommentKind::IssueComment => api::review::ExternalKind::IssueComment,
            PullCommentKind::Review => api::review::ExternalKind::Review,
        },
        url: external.url.clone(),
        path: external.path.clone(),
        pending: external.pending,
        summary: external.summary,
        vote: external.vote.map(|vote| match vote {
            Vote::Up => api::review::Vote::Up,
            Vote::Down => api::review::Vote::Down,
        }),
        lapsed: external.lapsed,
        agreed_reply: external.agreed_reply,
    })
}

pub(crate) fn batch_to_api(batch: &Batch) -> api::feedback::EventBody {
    api::feedback::EventBody::ExternalComments {
        comments: batch
            .comments
            .iter()
            .map(|d| comments::to_api(&d.comment))
            .collect(),
        edited: batch
            .comments
            .iter()
            .filter(|d| d.edited)
            .map(|d| d.comment.id.to_string())
            .collect(),
    }
}

/// How many comments `event` handed over, if it's a batch.
fn carried(event: &Event) -> usize {
    match &event.kind {
        EventKind::ExternalComments(batch) => batch.comments.len(),
        _ => 0,
    }
}

fn failure(e: ExternalError) -> Failure {
    use ExternalError as E;
    use StatusCode as S;
    let message = e.to_string();
    let (status, code, hint) = match e {
        E::Review(e) => return reviews::failure(e),
        E::Github(e) => return projects::github_failure(e),
        E::NotGithub(_) => (S::CONFLICT, "not_github", None),
        E::NoComment(_) => (S::NOT_FOUND, "comment_not_found", None),
        E::NotRejected(_) => (S::CONFLICT, "not_rejected", None),
        E::AlreadySent(_) => (S::CONFLICT, "already_sent", None),
        E::Summary(_) => (S::CONFLICT, "summary", None),
        E::Git(_) => (S::INTERNAL_SERVER_ERROR, "git_failed", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{message}");
    }
    Failure::new(status, code, message, hint)
}
