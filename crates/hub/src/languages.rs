//! `/api/languages`: the language packs the operator installs.

use std::path::PathBuf;
use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use mergegate_core::highlight::{Highlighter, LangError, Pack};

use crate::reply::Failure;
use crate::server::AppState;

type Reply<T> = Result<Json<T>, Failure>;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/languages", get(list).post(add))
        .route("/api/languages/{name}", delete(remove))
}

async fn list(State(state): State<Arc<AppState>>) -> Reply<api::lang::Packs> {
    Ok(Json(to_api(&state.languages.installed().packs)))
}

async fn add(
    State(state): State<Arc<AppState>>,
    request: Result<Json<api::lang::AddPacks>, JsonRejection>,
) -> Reply<api::lang::Packs> {
    let Json(request) = request?;
    let from = PathBuf::from(&request.from);
    // Paths are resolved by the client, which knows its working directory.
    if !from.is_absolute() {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "relative_path",
            format!("{:?} is not an absolute path", request.from),
            None,
        ));
    }
    // Compiling grammars takes a while; the store isn't held for it.
    let languages = Arc::clone(&state.languages);
    let installed = tokio::task::spawn_blocking(move || languages.install(&from))
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
        .map_err(failure)?;
    changed(&state);
    Ok(Json(to_api(&installed)))
}

async fn remove(
    State(state): State<Arc<AppState>>,
    name: Result<Path<String>, PathRejection>,
) -> Reply<api::lang::Pack> {
    let Path(name) = name?;
    let languages = Arc::clone(&state.languages);
    let removed = tokio::task::spawn_blocking(move || languages.remove(&name))
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
        .map_err(failure)?;
    changed(&state);
    Ok(Json(pack_to_api(&removed)))
}

/// The packs changed: open diffs refetch, and the pass highlights open items
/// again and drops the spans computed with the old packs.
fn changed(state: &AppState) {
    state.highlight.notify_one();
    state.events.send(UiEvent::HighlightsChanged);
}

fn to_api(packs: &[Pack]) -> api::lang::Packs {
    api::lang::Packs {
        packs: packs.iter().map(pack_to_api).collect(),
    }
}

fn pack_to_api(pack: &Pack) -> api::lang::Pack {
    api::lang::Pack {
        name: pack.name.clone(),
        version: pack.version.clone(),
        grammars: pack
            .grammars
            .iter()
            .map(|g| api::lang::Grammar {
                name: g.name.clone(),
                extensions: g.extensions.clone(),
                filenames: g.filenames.clone(),
            })
            .collect(),
    }
}

fn failure(e: LangError) -> Failure {
    use StatusCode as S;
    let (status, code, hint) = match &e {
        LangError::Invalid { .. } => (S::BAD_REQUEST, "invalid_pack", None),
        LangError::Empty(_) => (S::BAD_REQUEST, "no_packs", None),
        LangError::Taken { pack, .. } => (
            S::CONFLICT,
            "grammar_taken",
            Some(format!("mg lang remove {pack}")),
        ),
        LangError::NotInstalled(_) => (
            S::NOT_FOUND,
            "pack_not_installed",
            Some("mg lang list".into()),
        ),
        LangError::Failed(_) => (S::INTERNAL_SERVER_ERROR, "packs_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}
