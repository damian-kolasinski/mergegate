//! `/api/projects`: registering repos and changing their settings.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{self, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::github::GithubError;
use mergegate_core::highlight::{Highlighter, PackSource, available};
use mergegate_core::project::{
    self, AddProject, Added, Backend, Changed, Languages, Project, ProjectError, ProjectStore,
    Setting,
};

use crate::languages;
use crate::reply::{self, Failure};
use crate::server::AppState;

type Reply<T> = Result<Json<T>, Failure>;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/projects", get(list).post(add))
        .route("/api/projects/{name}", get(show).patch(set))
        // Not under /api/projects/, where it would shadow a project's name.
        .route("/api/project-at", get(at))
}

async fn list(State(state): State<Arc<AppState>>) -> Reply<api::project::Projects> {
    let projects = state
        .blocking(|store, _| store.projects())
        .await
        .map_err(|e| failure(e.into()))?;
    Ok(Json(api::project::Projects {
        projects: projects.iter().map(to_api).collect(),
    }))
}

async fn add(
    State(state): State<Arc<AppState>>,
    request: Result<Json<api::project::AddProject>, JsonRejection>,
) -> Result<Response, Failure> {
    let Json(request) = request?;
    let languages = Languages {
        known: available(&state.known_packs, state.releases.releases()),
        installed: state
            .languages
            .installed()
            .packs
            .into_iter()
            .map(|p| p.name)
            .collect(),
        chosen: request.languages,
    };
    let request = AddProject {
        path: absolute(&request.path)?,
        name: request.name,
        backend: request.backend.map(backend_from_api),
        target: request.target,
        github: request.github,
        languages,
    };
    let (project, names) = match state
        .blocking_github(move |store, git, github| project::add(store, git, github, request))
        .await
    {
        Ok(Added::Project { project, languages }) => (project, languages),
        Ok(Added::Decide(decision)) => return Ok(reply::decision(decision)),
        Err(e) => return Err(failure(e)),
    };
    // The project stays registered whatever happens to its packs, which
    // `mg lang add` installs again.
    let mut warnings = Vec::new();
    let installed = if names.is_empty() {
        Vec::new()
    } else {
        match languages::install_released(&state, names).await {
            Ok(installed) => {
                languages::changed(&state);
                installed
            }
            Err(e) => {
                warnings.push(format!(
                    "{e}; mg lang add --detect {} tries again",
                    project.name
                ));
                Vec::new()
            }
        }
    };
    let added = api::project::Added {
        project: to_api(&project),
        languages: languages::to_api(&installed).packs,
        warnings,
    };
    Ok((StatusCode::CREATED, Json(added)).into_response())
}

async fn show(
    State(state): State<Arc<AppState>>,
    name: Result<extract::Path<String>, PathRejection>,
) -> Reply<api::project::Project> {
    let extract::Path(name) = name?;
    let project = state
        .blocking(move |store, _| project::find(store, &name))
        .await
        .map_err(failure)?;
    Ok(Json(to_api(&project)))
}

async fn at(
    State(state): State<Arc<AppState>>,
    query: Result<Query<api::project::ProjectAt>, QueryRejection>,
) -> Reply<api::project::Project> {
    let Query(query) = query?;
    let path = absolute(&query.path)?;
    let project = state
        .blocking(move |store, git| project::at(store, git, &path))
        .await
        .map_err(failure)?;
    Ok(Json(to_api(&project)))
}

async fn set(
    State(state): State<Arc<AppState>>,
    name: Result<extract::Path<String>, PathRejection>,
    request: Result<Json<api::project::SetProject>, JsonRejection>,
) -> Result<Response, Failure> {
    let (extract::Path(name), Json(request)) = (name?, request?);
    let setting = Setting::parse(&request.key, &request.value).map_err(failure)?;
    if let Setting::Upstream(path) = &setting {
        absolute(&path.to_string_lossy())?;
    }
    let confirm = request.confirm;
    match state
        .blocking_github(move |store, git, github| {
            project::set(store, git, github, &name, setting, confirm)
        })
        .await
        .map_err(failure)?
    {
        Changed::Project(project) => Ok(Json(to_api(&project)).into_response()),
        Changed::Decide(decision) => Ok(reply::decision(decision)),
    }
}

fn to_api(project: &Project) -> api::project::Project {
    api::project::Project {
        name: project.name.to_string(),
        backend: backend_to_api(project.backend),
        target: project.target.clone(),
        upstream: project.upstream.display().to_string(),
        github: project.github.as_ref().map(ToString::to_string),
        bots: project.bots.clone(),
    }
}

pub(crate) fn backend_to_api(backend: Backend) -> api::project::Backend {
    match backend {
        Backend::Github => api::project::Backend::Github,
        Backend::Local => api::project::Backend::Local,
    }
}

fn backend_from_api(backend: api::project::Backend) -> Backend {
    match backend {
        api::project::Backend::Github => Backend::Github,
        api::project::Backend::Local => Backend::Local,
    }
}

/// Paths are resolved by the client, which knows its working directory; the
/// hub's own is meaningless to it.
fn absolute(path: &str) -> Result<PathBuf, Failure> {
    if Path::new(path).is_absolute() {
        return Ok(path.into());
    }
    Err(Failure::new(
        StatusCode::BAD_REQUEST,
        "relative_path",
        format!("{path:?} is not an absolute path"),
        None,
    ))
}

pub(crate) fn failure(e: ProjectError) -> Failure {
    use ProjectError as E;
    use StatusCode as S;
    let e = match e {
        E::Github(e) => return github_failure(e),
        e => e,
    };
    let (status, code, hint) = match &e {
        E::NotARepo(_) => (S::BAD_REQUEST, "not_a_repo", None),
        E::NoCommits(_) => (
            S::BAD_REQUEST,
            "no_commits",
            Some("commit to the repo, then add it again".into()),
        ),
        E::NoName(_) | E::InvalidName(_) => {
            (S::BAD_REQUEST, "invalid_name", Some("--name NAME".into()))
        }
        E::NameTaken(_) => (S::CONFLICT, "name_taken", Some("--name NAME".into())),
        E::AlreadyRegistered { project, .. } => (
            S::CONFLICT,
            "already_registered",
            Some(format!("mg project show {project}")),
        ),
        E::UnknownBranch { .. } => (S::BAD_REQUEST, "unknown_branch", None),
        E::BranchUnderReview { .. } => (S::CONFLICT, "branch_under_review", None),
        E::NoGithubRepo(_) => (
            S::BAD_REQUEST,
            "no_github_repo",
            Some("--github OWNER/REPO".into()),
        ),
        E::NoGithubSet(project) => (
            S::BAD_REQUEST,
            "no_github_repo",
            Some(format!("mg project set {project} github OWNER/REPO")),
        ),
        E::Github(_) => unreachable!("answered above"),
        E::NotFound(_) => (
            S::NOT_FOUND,
            "project_not_found",
            Some("mg project show".into()),
        ),
        E::NoProjectAt(_) => (
            S::NOT_FOUND,
            "no_project",
            Some("mg project add PATH".into()),
        ),
        E::UnknownSetting(_) | E::InvalidValue { .. } => (S::BAD_REQUEST, "invalid_setting", None),
        E::Git(_) => (S::INTERNAL_SERVER_ERROR, "git_failed", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}

/// GitHub's own errors, wherever they come up.
pub(crate) fn github_failure(e: GithubError) -> Failure {
    use GithubError as E;
    use StatusCode as S;
    let (status, code, hint) = match &e {
        E::NotLoggedIn(_) => (S::BAD_GATEWAY, "github_login", Some("gh auth login")),
        E::NoAccess(_) => (S::BAD_GATEWAY, "github_no_access", None),
        E::Refused(_) => (S::BAD_GATEWAY, "github_refused", None),
        E::Failed(_) => (S::BAD_GATEWAY, "github_failed", None),
    };
    tracing::warn!("{e}");
    Failure::new(status, code, e.to_string(), hint.map(Into::into))
}
