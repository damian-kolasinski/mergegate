//! `/api/projects`: registering repos and changing their settings.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{self, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::project::{
    self, AddProject, Added, Backend, Project, ProjectError, ProjectStore, Setting,
};

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
    let request = AddProject {
        path: absolute(&request.path)?,
        name: request.name,
        backend: request.backend.map(backend_from_api),
        target: request.target,
    };
    match state
        .blocking(move |store, git| project::add(store, git, request))
        .await
    {
        Ok(Added::Project(project)) => {
            Ok((StatusCode::CREATED, Json(to_api(&project))).into_response())
        }
        Ok(Added::Decide(decision)) => Ok(reply::decision(decision)),
        Err(e) => Err(failure(e)),
    }
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
) -> Reply<api::project::Project> {
    let (extract::Path(name), Json(request)) = (name?, request?);
    let setting = Setting::parse(&request.key, &request.value).map_err(failure)?;
    if let Setting::Upstream(path) = &setting {
        absolute(&path.to_string_lossy())?;
    }
    let project = state
        .blocking(move |store, git| project::set(store, git, &name, setting))
        .await
        .map_err(failure)?;
    Ok(Json(to_api(&project)))
}

fn to_api(project: &Project) -> api::project::Project {
    api::project::Project {
        name: project.name.to_string(),
        backend: backend_to_api(project.backend),
        target: project.target.clone(),
        upstream: project.upstream.display().to_string(),
    }
}

fn backend_to_api(backend: Backend) -> api::project::Backend {
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

fn failure(e: ProjectError) -> Failure {
    use ProjectError as E;
    use StatusCode as S;
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
        E::BackendUnavailable(_) => (
            S::BAD_REQUEST,
            "backend_unavailable",
            Some("use the local backend".into()),
        ),
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
