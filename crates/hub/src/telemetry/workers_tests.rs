use std::sync::{Arc, Mutex};
use std::time::Duration;

use api::diagnostics::Field;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, Registry};

use super::super::fields::Fields;
use super::spawn;

/// Each event as `event_code@worker`, the worker read from its span.
struct Codes(Arc<Mutex<Vec<String>>>);
impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Codes {
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
        let text = |key| match fields.0.get(key) {
            Some(Field::Text(text)) => text.clone(),
            _ => String::new(),
        };
        let code = format!("{}@{}", text("event_code"), text("worker"));
        self.0.lock().unwrap().push(code);
    }
}

fn run(work: impl Future<Output = ()> + Send + 'static) -> Vec<String> {
    let codes = Arc::new(Mutex::new(Vec::new()));
    let _guard =
        tracing::subscriber::set_default(Registry::default().with(Codes(Arc::clone(&codes))));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        spawn("test", work);
        tokio::time::sleep(Duration::from_millis(20)).await;
    });
    // As a stopping hub's does, with the worker still running.
    drop(runtime);
    codes.lock().unwrap().clone()
}

#[test]
fn a_worker_dropped_by_a_stopping_runtime_is_recorded_as_cancelled() {
    assert_eq!(
        run(std::future::pending()),
        ["worker_started@test", "worker_cancelled@test"]
    );
}

#[test]
fn a_finished_worker_is_not_cancelled() {
    assert_eq!(
        run(async {}),
        ["worker_started@test", "worker_finished@test"]
    );
}

#[test]
fn a_panicking_worker_is_recorded_once() {
    assert_eq!(
        run(async { panic!("worker failed") }),
        ["worker_started@test", "worker_panicked@test"]
    );
}
