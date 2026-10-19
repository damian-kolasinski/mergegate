//! `/api/languages`: the language packs the operator installs, from files
//! or from mg's release.

use std::path::PathBuf;
use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use mergegate_core::highlight::{Highlighter, LangError, Pack, PackSource, available, outdated};
use mergegate_core::project::{self, ProjectError};

use crate::projects;
use crate::reply::Failure;
use crate::server::AppState;

type Reply<T> = Result<Json<T>, Failure>;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/languages", get(list).post(add))
        .route("/api/languages/{name}", delete(remove))
        // Not under /api/languages/, where it would shadow a pack's name.
        .route("/api/released-languages", get(released))
}

async fn list(State(state): State<Arc<AppState>>) -> Reply<api::lang::Packs> {
    Ok(Json(to_api(&state.languages.installed().packs)))
}

async fn released(State(state): State<Arc<AppState>>) -> Reply<api::lang::Released> {
    let installed = state.languages.installed();
    let releases = state.releases.releases();
    let outdated = outdated(&installed, releases);
    let packs = available(&state.known_packs, releases)
        .iter()
        .map(|p| api::lang::ReleasedPack {
            pack: pack_to_api(p),
            installed: installed.packs.iter().any(|i| i.name == p.name),
            outdated: outdated.contains(&p.name),
        })
        .collect();
    Ok(Json(api::lang::Released { packs }))
}

enum Source {
    From(PathBuf),
    Released(Vec<String>),
}

async fn add(
    State(state): State<Arc<AppState>>,
    request: Result<Json<api::lang::AddPacks>, JsonRejection>,
) -> Reply<api::lang::Packs> {
    let Json(request) = request?;
    let installed = match source(&state, request).await? {
        Source::From(from) => {
            // Compiling grammars takes a while; the store isn't held for it.
            let languages = Arc::clone(&state.languages);
            tokio::task::spawn_blocking(move || languages.install(&from))
                .await
                .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
        }
        Source::Released(names) => install_released(&state, names).await,
    }
    .map_err(failure)?;
    if !installed.is_empty() {
        changed(&state);
    }
    Ok(Json(to_api(&installed)))
}

async fn source(state: &Arc<AppState>, request: api::lang::AddPacks) -> Result<Source, Failure> {
    let api::lang::AddPacks {
        from,
        names,
        detect,
        update,
    } = request;
    let sources = usize::from(from.is_some())
        + usize::from(!names.is_empty())
        + usize::from(detect.is_some())
        + usize::from(update);
    if sources != 1 {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "give exactly one of from, names, detect and update",
            None,
        ));
    }
    if let Some(from) = from {
        // Paths are resolved by the client, which knows its working directory.
        if !PathBuf::from(&from).is_absolute() {
            return Err(Failure::new(
                StatusCode::BAD_REQUEST,
                "relative_path",
                format!("{from:?} is not an absolute path"),
                None,
            ));
        }
        return Ok(Source::From(from.into()));
    }
    let installed = state.languages.installed();
    if update {
        return Ok(Source::Released(outdated(
            &installed,
            state.releases.releases(),
        )));
    }
    let Some(name) = detect else {
        return Ok(Source::Released(names));
    };
    let known = available(&state.known_packs, state.releases.releases());
    let installed: Vec<String> = installed.packs.into_iter().map(|p| p.name).collect();
    let missing = state
        .blocking(move |store, git| {
            let project = project::find(store, &name)?;
            project::missing(git, &project.upstream, &project.target, &known, &installed)
                .map_err(ProjectError::from)
        })
        .await
        .map_err(projects::failure)?;
    Ok(Source::Released(
        missing.into_iter().map(|d| d.pack).collect(),
    ))
}

/// Downloads again, once at start, the packs released for another mg, as an
/// upgrade leaves them: their grammars may not load in this one, and nothing
/// else would replace them until the operator ran `mg lang update`.
pub(crate) async fn refresh(state: Arc<AppState>) {
    let names = outdated(&state.languages.installed(), state.releases.releases());
    if names.is_empty() {
        return;
    }
    match install_released(&state, names).await {
        Ok(packs) => {
            let names: Vec<&str> = packs.iter().map(|p| p.name.as_str()).collect();
            tracing::info!("updated the language packs {}", names.join(", "));
            changed(&state);
        }
        Err(e) => tracing::warn!("updating the language packs: {e}; mg lang update tries again"),
    }
}

/// Downloads `names` from mg's release and installs them, all or none.
pub(crate) async fn install_released(
    state: &Arc<AppState>,
    names: Vec<String>,
) -> Result<Vec<Pack>, LangError> {
    let state = Arc::clone(state);
    tokio::task::spawn_blocking(move || {
        let into = tempfile::tempdir().map_err(|e| LangError::Failed(Box::new(e)))?;
        mergegate_core::highlight::install(&*state.releases, &*state.languages, &names, into.path())
    })
    .await
    .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
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
pub(crate) fn changed(state: &AppState) {
    state.highlight.notify_one();
    state.events.send(UiEvent::HighlightsChanged);
}

pub(crate) fn to_api(packs: &[Pack]) -> api::lang::Packs {
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

pub(crate) fn failure(e: LangError) -> Failure {
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
        LangError::NoReleases => (
            S::BAD_REQUEST,
            "no_releases",
            Some("mg lang add --from PATH".into()),
        ),
        LangError::NotReleased(_) => (
            S::NOT_FOUND,
            "pack_not_released",
            Some("mg lang list --available".into()),
        ),
        LangError::Download { .. } => (S::BAD_GATEWAY, "download_failed", None),
        LangError::Failed(_) => (S::INTERNAL_SERVER_ERROR, "packs_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}
