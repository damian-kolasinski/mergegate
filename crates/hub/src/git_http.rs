//! `/git/{project}`: the hub as a git remote. Workers push their branches to
//! the project's staging repo and fetch its target, over smart HTTP served by
//! `git http-backend`.

use std::process::Stdio;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use futures_util::StreamExt;
use git::HttpRequest;
use mergegate_core::item::ItemGit;
use mergegate_core::project::{ProjectError, ProjectName, ProjectStore};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdout, Command};

use crate::projects;
use crate::reply::Failure;
use crate::server::AppState;

/// The request body types of `git fetch` and `git push`. A browser can't send
/// them cross-site without a preflight, so they pass the guard as the
/// client header does.
pub(crate) const REQUEST_TYPES: &[&str] = &[
    "application/x-git-upload-pack-request",
    "application/x-git-receive-pack-request",
];

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/git/{project}/{*rest}", any(serve))
}

async fn serve(
    State(state): State<Arc<AppState>>,
    Path((name, rest)): Path<(String, String)>,
    req: Request,
) -> Result<Response, Failure> {
    let method = req.method().clone();
    let query = req.uri().query().unwrap_or_default().to_owned();
    let path = format!("/{rest}");
    let headers = req.headers().clone();
    // A fetch starts by listing refs; the target is fetched from the
    // upstream first, so workers rebase onto what's really there.
    let fetching = method == Method::GET
        && path == "/info/refs"
        && query.split('&').any(|p| p == "service=git-upload-pack");
    let command = state
        .blocking(move |store, git| {
            let name = ProjectName::parse(&name).map_err(|_| ProjectError::NotFound(name))?;
            let project = store
                .project(&name)?
                .ok_or_else(|| ProjectError::NotFound(name.to_string()))?;
            if fetching
                && let Err(e) = git.fetch_target(&project.name, &project.upstream, &project.target)
            {
                // Serve what staging has; the worker sees a stale target at worst.
                tracing::warn!("{e}");
            }
            let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
            let request = HttpRequest {
                method: method.as_str(),
                path: &path,
                query: &query,
                content_type: header("content-type"),
                content_length: header("content-length"),
                content_encoding: header("content-encoding"),
                git_protocol: header("git-protocol"),
            };
            Ok::<_, ProjectError>(git.http_backend(&project.name, &request))
        })
        .await
        .map_err(projects::failure)?;
    run(Command::from(command), req.into_body()).await
}

/// Runs `git http-backend` with the request body on its stdin and answers
/// with what it prints, both streamed.
async fn run(mut command: Command, body: Body) -> Result<Response, Failure> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| backend_failed(&format!("can't run git http-backend: {e}")))?;
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        unreachable!("both are piped");
    };
    tokio::spawn(async move {
        let mut body = body.into_data_stream();
        while let Some(Ok(chunk)) = body.next().await {
            if stdin.write_all(&chunk).await.is_err() {
                break;
            }
        }
        // Dropping stdin ends the request for git.
    });
    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    let mut stdout = BufReader::new(stdout);
    let (status, headers) = cgi_headers(&mut stdout)
        .await
        .ok_or_else(|| backend_failed("git http-backend gave no CGI headers"))?;
    let body = futures_util::stream::try_unfold(stdout, |mut stdout| async move {
        let mut buf = vec![0; 64 * 1024];
        let n = stdout.read(&mut buf).await?;
        buf.truncate(n);
        Ok::<_, std::io::Error>((n > 0).then(|| (Bytes::from(buf), stdout)))
    });
    Ok((status, headers, Body::from_stream(body)).into_response())
}

/// Reads CGI headers up to the blank line that ends them. `Status` sets the
/// response's status; the rest are HTTP headers.
async fn cgi_headers(stdout: &mut BufReader<ChildStdout>) -> Option<(StatusCode, HeaderMap)> {
    let mut status = StatusCode::OK;
    let mut headers = HeaderMap::new();
    loop {
        let mut line = String::new();
        if stdout.read_line(&mut line).await.ok()? == 0 {
            return None;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return Some((status, headers));
        }
        let (name, value) = line.split_once(':')?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("status") {
            let code = value.split(' ').next()?;
            status = StatusCode::from_bytes(code.as_bytes()).ok()?;
        } else {
            headers.append(
                HeaderName::from_bytes(name.as_bytes()).ok()?,
                HeaderValue::from_str(value).ok()?,
            );
        }
    }
}

fn backend_failed(message: &str) -> Failure {
    tracing::error!("{message}");
    Failure::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "git_failed",
        message,
        None,
    )
}
