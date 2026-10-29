//! Bounded local evidence, with private message text excluded from default exports.

pub(crate) mod disk;
mod fields;
mod requests;
mod workers;

pub(crate) use requests::record_request;
pub(crate) use workers::spawn;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use api::diagnostics::Record;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, Registry};

use crate::Home;

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub(crate) struct Guard {
    stopping: Arc<AtomicBool>,
    done: mpsc::Receiver<()>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        // A stalled disk must not prevent the daemon from stopping.
        let _ = self.done.recv_timeout(Duration::from_secs(2));
    }
}

struct Recorder {
    sender: SyncSender<Record>,
    stopping: Arc<AtomicBool>,
    sequence: AtomicU64,
    dropped: Arc<AtomicU64>,
    run_id: String,
    version: String,
}

pub(crate) fn init(home: &Home, version: &str) -> Guard {
    let (sender, receiver) = mpsc::sync_channel(256);
    let (finished, done) = mpsc::channel();
    let stopping = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicU64::new(0));
    let stop = Arc::clone(&stopping);
    let lost = Arc::clone(&dropped);
    let directory = home.root().join("telemetry");
    let _ = std::thread::Builder::new()
        .name("telemetry".into())
        .spawn(move || {
            disk::run(directory, receiver, &stop, &lost);
            let _ = finished.send(());
        });
    let recorder = Recorder {
        sender,
        stopping: Arc::clone(&stopping),
        sequence: AtomicU64::new(0),
        version: version.to_owned(),
        dropped,
        run_id: format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ),
    };
    use std::io::IsTerminal;
    // The readable log a backgrounded hub's service sends to `hub.log`.
    // Request records stay out of it: they are most of the volume and only
    // useful as correlated evidence.
    let console = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .with_target(false)
        .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
            *meta.level() <= tracing::Level::INFO && meta.target() != requests::TARGET
        }));
    let _ = tracing::subscriber::set_global_default(
        Registry::default()
            .with(recorder.with_filter(tracing_subscriber::filter::LevelFilter::INFO))
            .with(console),
    );
    std::panic::set_hook(Box::new(|panic| {
        // The recorder bounds and sanitizes the payload as private detail;
        // default exports retain only the panic's location and event code.
        let message = panic
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        let line = panic.location().map_or(0, |l| l.line());
        let source_file = panic
            .location()
            .and_then(|l| std::path::Path::new(l.file()).file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        tracing::error!(
            event_code = "panic",
            source_line = line,
            source_file,
            "{message}"
        );
    }));
    tracing::info!(
        event_code = "hub_starting",
        version,
        api_fingerprint = api::API_VERSION
    );
    Guard { stopping, done }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Recorder {
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: Context<'_, S>,
    ) {
        let mut fields = fields::Fields::default();
        attrs.record(&mut fields);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(fields);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if self.stopping.load(Ordering::Relaxed) {
            return;
        }
        let mut fields = BTreeMap::new();
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope.from_root() {
                if let Some(parent) = span.extensions().get::<fields::Fields>() {
                    fields.extend(parent.0.clone());
                }
            }
        }
        let mut own = fields::Fields::default();
        event.record(&mut own);
        fields.extend(own.0);
        let meta = event.metadata();
        let record = Record {
            timestamp: now(),
            run_id: self.run_id.clone(),
            version: self.version.clone(),
            detail: own.1,
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
            level: meta.level().to_string(),
            target: meta.target().chars().take(128).collect(),
            line: meta.line(),
            fields,
            dropped: self.dropped.load(Ordering::Relaxed),
        };
        if self.sender.try_send(record).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
