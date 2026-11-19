use std::sync::{Arc, Mutex};
use std::time::Duration;

use api::diagnostics::Field as Value;
use axum::Router;
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::get;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, Registry};

use super::super::fields::Fields;
use super::{record_request, waits};

type Records = Arc<Mutex<Vec<Fields>>>;

/// Each event's own fields merged with its request span's.
struct Capture(Records);
impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Capture {
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: Context<'_, S>,
    ) {
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        ctx.span(id).unwrap().extensions_mut().insert(fields);
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope.from_root() {
                if let Some(parent) = span.extensions().get::<Fields>() {
                    fields.0.extend(parent.0.clone());
                }
            }
        }
        event.record(&mut fields);
        self.0.lock().unwrap().push(fields);
    }
}

fn text(value: &str) -> Value {
    Value::Text(value.into())
}

/// Serves the middleware on loopback and records what it emits on this
/// thread, which a current-thread test runtime runs every task on.
async fn serve(records: &Records) -> (std::net::SocketAddr, tracing::subscriber::DefaultGuard) {
    let guard =
        tracing::subscriber::set_default(Registry::default().with(Capture(Arc::clone(records))));
    let app = Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route(
            "/api/inbox",
            get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
        )
        .route("/api/items/{id}", get(|| async { "item" }))
        .route("/api/items/{id}/feedback", get(|| async { "events" }))
        .route("/api/wakes", get(|| async { StatusCode::BAD_REQUEST }))
        .route("/api/stuck", get(std::future::pending::<String>))
        .route(
            "/api/broken",
            get(|| async {
                panic!("handler failed");
                #[allow(unreachable_code)]
                ""
            }),
        )
        .layer(middleware::from_fn(record_request));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await });
    (addr, guard)
}

async fn get_path(addr: std::net::SocketAddr, path: &str) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: hub\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    stream.read_to_end(&mut Vec::new()).await.unwrap();
}

fn codes(records: &Records) -> Vec<(Value, Value)> {
    records
        .lock()
        .unwrap()
        .iter()
        .map(|f| (f.0["event_code"].clone(), f.0["route"].clone()))
        .collect()
}

#[tokio::test]
async fn polled_routes_are_recorded_only_when_they_fail() {
    let records = Records::default();
    let (addr, _guard) = serve(&records).await;
    get_path(addr, "/api/health").await;
    get_path(addr, "/api/inbox").await;
    get_path(addr, "/api/items/7").await;
    assert_eq!(
        codes(&records),
        [
            (text("request_finished"), text("/api/inbox")),
            (text("request_finished"), text("/api/items/{id}")),
        ]
    );
    let records = records.lock().unwrap();
    assert_eq!(records[0].0["status"], Value::Number(500));
    assert!(matches!(records[1].0["elapsed_ms"], Value::Number(_)));
}

#[tokio::test]
async fn a_request_that_waits_is_a_long_poll() {
    let records = Records::default();
    let (addr, _guard) = serve(&records).await;
    get_path(addr, "/api/items/7/feedback?wait=300").await;
    get_path(addr, "/api/items/7/feedback?wait=0").await;
    get_path(addr, "/api/wakes?wait=300").await;
    assert_eq!(
        codes(&records),
        [
            (text("long_poll_finished"), text("/api/items/{id}/feedback")),
            (text("request_finished"), text("/api/items/{id}/feedback")),
            (text("request_finished"), text("/api/wakes")),
        ]
    );
}

#[tokio::test]
async fn a_request_whose_client_leaves_is_recorded_as_cancelled() {
    let records = Records::default();
    let (addr, _guard) = serve(&records).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET /api/stuck HTTP/1.1\r\nHost: hub\r\n\r\n")
        .await
        .unwrap();
    // Let the hub start handling it before the client goes.
    tokio::time::sleep(Duration::from_millis(50)).await;
    drop(stream);
    tokio::time::timeout(Duration::from_secs(5), async {
        while codes(&records).is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a cancelled request is recorded");
    assert_eq!(
        codes(&records),
        [(text("request_cancelled"), text("/api/stuck"))]
    );
}

#[tokio::test]
async fn a_panicking_handler_is_not_mistaken_for_a_client_leaving() {
    let records = Records::default();
    let (addr, _guard) = serve(&records).await;
    get_path(addr, "/api/broken").await;
    assert_eq!(
        codes(&records),
        [(text("request_panicked"), text("/api/broken"))]
    );
}

#[test]
fn only_a_positive_wait_is_a_long_poll() {
    assert!(waits(Some("wait=300")));
    assert!(waits(Some("after=4&wait=1")));
    assert!(!waits(Some("wait=0")));
    assert!(!waits(Some("wait=")));
    assert!(!waits(Some("nowait=5")));
    assert!(!waits(None));
}
