use std::future::Future;

use tracing::Instrument;

pub(crate) fn spawn(name: &'static str, work: impl Future<Output = ()> + Send + 'static) {
    let span = tracing::info_span!("worker", worker = name);
    let task = tokio::spawn(
        async move {
            let mut running = Running(true);
            tracing::info!(event_code = "worker_started");
            work.await;
            running.0 = false;
            tracing::info!(event_code = "worker_finished");
        }
        .instrument(span),
    );
    tokio::spawn(async move {
        if let Err(error) = task.await
            && error.is_panic()
        {
            tracing::error!(event_code = "worker_panicked", worker = name);
        }
    });
}

/// Records a worker dropped before it finished. A stopping hub's runtime
/// drops the workers that never return, and this watcher with them, so only
/// the dropped future itself can say so, inside the worker's span, which an
/// instrumented future enters while it is dropped.
struct Running(bool);

impl Drop for Running {
    fn drop(&mut self) {
        // An unwinding worker is reported as panicked by its watcher.
        if self.0 && !std::thread::panicking() {
            tracing::info!(event_code = "worker_cancelled");
        }
    }
}

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;
