use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderValue, header::HeaderName};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;

static NEXT: AtomicU64 = AtomicU64::new(1);

pub(crate) async fn record_request(request: Request, next: Next) -> Response {
    let request_id = format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("unmatched", MatchedPath::as_str);
    let span = tracing::info_span!(
        "request",
        request_id = request_id.as_str(),
        route,
        method = request.method().as_str()
    );
    let started = Instant::now();
    let mut response = async {
        tracing::info!(event_code = "request_started");
        let response = next.run(request).await;
        tracing::info!(
            event_code = "request_finished",
            status = u64::from(response.status().as_u16()),
            elapsed_ms = started.elapsed().as_millis() as u64
        );
        response
    }
    .instrument(span)
    .await;
    response.headers_mut().insert(
        HeaderName::from_static("x-mergegate-request-id"),
        HeaderValue::from_str(&request_id).expect("numeric request ID"),
    );
    response
}
