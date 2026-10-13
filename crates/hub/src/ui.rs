//! `GET /` and `GET /assets/*`: the UI build.rs built into the binary. With
//! `MG_UI_DIR` set, files come from that directory instead, read on every
//! request, so a `pnpm watch` rebuild shows on reload without rebuilding mg.

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, LazyLock};

use axum::Router;
use axum::extract::rejection::PathRejection;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, ETAG, IF_NONE_MATCH,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use mergegate_core::doctor::{Check, CheckStatus};

use crate::reply::Failure;
use crate::server::AppState;

pub(crate) const DIR_ENV: &str = "MG_UI_DIR";

/// The UI talks only to this hub and loads nothing from elsewhere, so a
/// script injected through rendered agent text can't reach out either.
const POLICY: &str = "default-src 'self'; connect-src 'self'; img-src 'self' data:; \
     style-src 'self'; font-src 'self'; frame-ancestors 'none'; base-uri 'none'; \
     form-action 'none'";

struct Asset {
    /// Relative to the site root, `/`-separated.
    path: &'static str,
    etag: &'static str,
    body: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));

static DEV_DIR: LazyLock<Option<PathBuf>> =
    LazyLock::new(|| std::env::var_os(DIR_ENV).map(PathBuf::from));

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(index))
        .route("/assets/{*path}", get(asset))
}

async fn index(request: HeaderMap) -> Response {
    serve("index.html", &request).await
}

async fn asset(
    path: Result<axum::extract::Path<String>, PathRejection>,
    request: HeaderMap,
) -> Response {
    match path {
        Ok(path) => serve(&format!("assets/{}", path.0), &request).await,
        Err(rejection) => Failure::from(rejection).into_response(),
    }
}

/// The `ui` check of `mg doctor`.
pub(crate) fn check() -> Check {
    let (status, message, fix) = match (&*DEV_DIR, BUILT) {
        (Some(dir), _) => (
            CheckStatus::Ok,
            format!("served from {} ({DIR_ENV})", dir.display()),
            None,
        ),
        (None, true) => (CheckStatus::Ok, "built in".to_owned(), None),
        (None, false) => (
            CheckStatus::Warn,
            "this mg was built without its UI".to_owned(),
            Some("install pnpm, then rebuild mg with MG_REQUIRE_UI=1".to_owned()),
        ),
    };
    Check {
        name: "ui".into(),
        status,
        message,
        fix,
    }
}

async fn serve(path: &str, request: &HeaderMap) -> Response {
    if let Some(dir) = &*DEV_DIR {
        return from_disk(dir, path).await;
    }
    let Some(asset) = ASSETS.iter().find(|a| a.path == path) else {
        return not_found();
    };
    let mut headers = headers(path);
    headers.insert(ETAG, HeaderValue::from_static(asset.etag));
    // `no-cache` still stores the file but asks first, so a rebuilt mg is
    // never served a stale bundle under the same name.
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    if is_fresh(request, asset.etag) {
        return (StatusCode::NOT_MODIFIED, headers).into_response();
    }
    (headers, asset.body).into_response()
}

async fn from_disk(dir: &Path, path: &str) -> Response {
    if !is_plain(path) {
        return not_found();
    }
    let Ok(body) = tokio::fs::read(dir.join(path)).await else {
        return not_found();
    };
    let mut headers = headers(path);
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    (headers, body).into_response()
}

fn headers(path: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type(path)));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    if path == "index.html" {
        headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(POLICY));
    }
    headers
}

fn is_fresh(request: &HeaderMap, etag: &str) -> bool {
    request
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .any(|tag| tag == "*" || tag == etag)
}

/// Relative, with no `..`: a path under the directory it is joined to.
fn is_plain(path: &str) -> bool {
    Path::new(path)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, ext)| ext) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("woff2") => "font/woff2",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

fn not_found() -> Response {
    Failure::new(StatusCode::NOT_FOUND, "not_found", "no such file", None).into_response()
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
