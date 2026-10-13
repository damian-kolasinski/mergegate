use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::git_http;
use crate::reply::Failure;
use crate::server::AppState;

/// Accepts only requests addressed to the loopback interface (against DNS
/// rebinding) and mutating requests that carry the client header or come
/// from git (against cross-site requests from a browser).
pub(crate) async fn guard(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    let port = state.port;
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        return forbidden("requests must address the hub as 127.0.0.1 or localhost");
    }
    if req.method() != Method::GET
        && !req.headers().contains_key(api::CLIENT_HEADER)
        && !from_git(&req)
    {
        return forbidden(&format!(
            "requests must carry the {} header",
            api::CLIENT_HEADER
        ));
    }
    next.run(req).await
}

/// A `git fetch` or `git push` to the hub's remote, which can't carry the
/// client header but sends a body type browsers can't send cross-site.
fn from_git(req: &Request) -> bool {
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    req.uri().path().starts_with("/git/") && git_http::REQUEST_TYPES.contains(&content_type)
}

fn forbidden(message: &str) -> Response {
    Failure::new(StatusCode::FORBIDDEN, "forbidden", message, None).into_response()
}
