use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderValue, header::HeaderName};
use axum::middleware::Next;
use axum::response::Response;
use tracing::{Instrument, Span};

pub(crate) const TARGET: &str = module_path!();

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Routes clients poll many times an hour, recorded only when they fail or
/// are slow, as otherwise they bury every other record.
const POLLED: [&str; 3] = ["/api/health", "/api/inbox", "/api/names/{name}"];
const SLOW: Duration = Duration::from_secs(1);

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
    let polled = POLLED.contains(&route);
    let waits = waits(request.uri().query());
    let span = tracing::info_span!(
        "request",
        request_id = request_id.as_str(),
        route,
        method = request.method().as_str()
    );
    let mut pending = Pending {
        span: span.clone(),
        started: Instant::now(),
        finished: false,
    };
    let mut response = next.run(request).instrument(span.clone()).await;
    pending.finished = true;
    let elapsed = pending.started.elapsed();
    let succeeded = response.status().is_success();
    let status = u64::from(response.status().as_u16());
    span.in_scope(|| {
        let elapsed_ms = elapsed.as_millis() as u64;
        // A long poll's duration is how long it waited for news, not latency;
        // one refused before waiting is an ordinary failed request.
        if waits && succeeded {
            tracing::info!(event_code = "long_poll_finished", status, elapsed_ms);
        } else if !polled || elapsed >= SLOW || !succeeded {
            tracing::info!(event_code = "request_finished", status, elapsed_ms);
        }
    });
    response.headers_mut().insert(
        HeaderName::from_static("x-mergegate-request-id"),
        HeaderValue::from_str(&request_id).expect("numeric request ID"),
    );
    response
}

/// Whether the request asks to wait for news, as `mg wait` does through
/// `?wait=` on feedback and wakes.
fn waits(query: Option<&str>) -> bool {
    query.is_some_and(|query| {
        query
            .split('&')
            .filter_map(|pair| pair.strip_prefix("wait="))
            .any(|seconds| !seconds.is_empty() && seconds != "0")
    })
}

/// Records a request whose client went away before it was answered, which
/// drops the handler mid-flight: an ended `mg wait`, a closed UI tab. A
/// panicking handler is dropped too, while unwinding.
struct Pending {
    span: Span,
    started: Instant,
    finished: bool,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if !self.finished {
            let elapsed_ms = self.started.elapsed().as_millis() as u64;
            self.span.in_scope(|| {
                if std::thread::panicking() {
                    tracing::error!(event_code = "request_panicked", elapsed_ms);
                } else {
                    tracing::info!(event_code = "request_cancelled", elapsed_ms);
                }
            });
        }
    }
}

#[cfg(test)]
#[path = "requests_tests.rs"]
mod tests;
