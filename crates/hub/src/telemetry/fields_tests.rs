use std::sync::{Arc, Mutex};

use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::{Layer, Registry};

use super::{Fields, Value};

struct Capture(Arc<Mutex<Fields>>);
impl<S: Subscriber> Layer<S> for Capture {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        event.record(&mut *self.0.lock().unwrap());
    }
}

#[test]
fn only_operational_fields_enter_the_recorder() {
    let captured = Arc::new(Mutex::new(Fields::default()));
    let subscriber = Registry::default().with(Capture(Arc::clone(&captured)));
    tracing::subscriber::with_default(subscriber, || {
        tracing::error!(
            event_code = "request_failed",
            error_code = "store_failed",
            status = 500_u64,
            token = "secret",
            body = "source code",
            route = "/api/items/{id}",
            "arbitrary message ghp_secret"
        );
        tracing::info!(
            worker = "bad worker with secrets",
            request_id = "injected\nsecret"
        );
    });
    let fields = &captured.lock().unwrap().0;
    assert_eq!(fields.len(), 4, "{fields:?}");
    assert_eq!(fields["event_code"], Value::Text("request_failed".into()));
    assert_eq!(fields["status"], Value::Number(500));
    assert_eq!(fields["route"], Value::Text("/api/items/{id}".into()));
}

#[test]
fn details_are_bounded_redacted_and_separate_from_operational_fields() {
    let captured = Arc::new(Mutex::new(Fields::default()));
    let subscriber = Registry::default().with(Capture(Arc::clone(&captured)));
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!("failure Authorization: Bearer secret");
    });
    assert_eq!(
        captured.lock().unwrap().1.as_deref(),
        Some("failure [redacted]")
    );
    let subscriber = Registry::default().with(Capture(Arc::clone(&captured)));
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(message = "ż".repeat(100_000).as_str());
    });
    assert!(captured.lock().unwrap().1.as_ref().unwrap().len() <= 1024);
    assert!(captured.lock().unwrap().0.is_empty());
}
