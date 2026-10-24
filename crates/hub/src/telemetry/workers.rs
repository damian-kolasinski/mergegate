use std::future::Future;

use tracing::Instrument;

pub(crate) fn spawn(name: &'static str, work: impl Future<Output = ()> + Send + 'static) {
    let span = tracing::info_span!("worker", worker = name);
    let task = tokio::spawn(
        async move {
            tracing::info!(event_code = "worker_started");
            work.await;
            tracing::info!(event_code = "worker_finished");
        }
        .instrument(span),
    );
    tokio::spawn(async move {
        if let Err(error) = task.await {
            let event_code = if error.is_panic() {
                "worker_panicked"
            } else {
                "worker_cancelled"
            };
            tracing::error!(event_code, worker = name);
        }
    });
}
