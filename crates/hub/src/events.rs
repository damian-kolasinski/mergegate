//! `GET /api/events/ui`: tells open UIs what changed, so they refetch it.
//! Also wakes the agents' waits when the event log grows.

use std::convert::Infallible;
use std::sync::Arc;

use api::event::UiEvent;
use axum::Router;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use futures_util::Stream;
use tokio::sync::{broadcast, watch};

use crate::server::AppState;

/// Fans events out to every open stream.
pub(crate) struct Events {
    sender: broadcast::Sender<UiEvent>,
    /// Counts appends to the event log; a waiting agent re-reads on a change.
    logged: watch::Sender<u64>,
    stopping: watch::Sender<bool>,
}

impl Events {
    pub(crate) fn new() -> Self {
        Self {
            sender: broadcast::channel(64).0,
            logged: watch::Sender::new(0),
            stopping: watch::Sender::new(false),
        }
    }

    pub(crate) fn send(&self, event: UiEvent) {
        // Fails only when no UI is open, and then there's no one to tell.
        let _ = self.sender.send(event);
    }

    /// Call after a write that appended to the event log commits.
    pub(crate) fn logged(&self) {
        self.logged
            .send_modify(|count| *count = count.wrapping_add(1));
    }

    /// Changes after each append to the event log. Mark it seen before
    /// reading the log, so an append during the read isn't missed.
    pub(crate) fn log(&self) -> watch::Receiver<u64> {
        self.logged.subscribe()
    }

    pub(crate) fn stopping(&self) -> watch::Receiver<bool> {
        self.stopping.subscribe()
    }

    /// Ends every stream and wait. Graceful shutdown waits for open
    /// responses, and these don't finish on their own.
    pub(crate) fn stop(&self) {
        self.stopping.send_replace(true);
    }
}

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/events/ui", get(stream))
}

async fn stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receivers = (state.events.sender.subscribe(), state.events.stopping());
    let stream = futures_util::stream::unfold(receivers, |(mut events, mut stopping)| async move {
        let event = tokio::select! {
            _ = stopping.wait_for(|stopping| *stopping) => return None,
            // A stream that fell behind ends instead of skipping events: the
            // browser reconnects and the UI refetches everything on connect.
            received = events.recv() => received.ok()?,
        };
        let data = serde_json::to_string(&event).expect("a UI event serializes");
        Some((Ok(Event::default().data(data)), (events, stopping)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
