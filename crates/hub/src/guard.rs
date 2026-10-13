use std::sync::Arc;

use axum::Json;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::server::AppState;

/// Accepts only requests addressed to the loopback interface (against DNS
/// rebinding) and mutating requests that carry the client header (against
/// cross-site requests from a browser).
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
    if req.method() != Method::GET && !req.headers().contains_key(api::CLIENT_HEADER) {
        return forbidden(&format!(
            "requests must carry the {} header",
            api::CLIENT_HEADER
        ));
    }
    next.run(req).await
}

fn forbidden(message: &str) -> Response {
    let body = api::error::ErrorEnvelope {
        error: api::error::ErrorBody {
            code: "forbidden".into(),
            message: message.into(),
            hint: None,
        },
    };
    (StatusCode::FORBIDDEN, Json(body)).into_response()
}
