use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::middleware;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::Notify;

use git::HubGit;
use store_sqlite::Store;

use crate::{
    Home, HubError, HubInfo, comments, conversation, diff, doctor, draft_replies, events, feedback,
    git_http, guard, io, items, lifecycle, projects, replies, reply, reviews, ui, viewed,
};

pub(crate) struct AppState {
    pub health: api::hub::Health,
    pub port: u16,
    pub shutdown: Notify,
    pub events: events::Events,
    /// Held for a whole request, which also keeps two requests from syncing
    /// the same staging repo at once.
    store: Mutex<Store>,
    git: HubGit,
}

impl AppState {
    /// Runs `f` off the async workers: SQLite and git block.
    pub(crate) async fn blocking<T: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&mut Store, &HubGit) -> T + Send + 'static,
    ) -> T {
        let state = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            // A panic mid-request leaves nothing half-applied: every write is
            // a single statement.
            let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut store, &state.git)
        })
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }
}

pub(crate) async fn serve(
    home: &Home,
    port: u16,
    schema_version: u32,
    store: Store,
) -> Result<(), HubError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AddrInUse => HubError::PortInUse(port),
            _ => io("binding the hub's port")(e),
        })?;
    let addr = listener
        .local_addr()
        .map_err(io("reading the bound address"))?;
    let pid = std::process::id();
    let state = Arc::new(AppState {
        health: api::hub::Health {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            api_version: api::API_VERSION,
            schema_version,
            pid,
            home: home.root().display().to_string(),
        },
        port: addr.port(),
        shutdown: Notify::new(),
        events: events::Events::new(),
        store: Mutex::new(store),
        git: HubGit::new(home.projects(), home.git_home()),
    });
    let app = Router::new()
        .merge(lifecycle::routes())
        .merge(projects::routes())
        .merge(items::routes())
        .merge(diff::routes())
        .merge(conversation::routes())
        .merge(viewed::routes())
        .merge(reviews::routes())
        .merge(comments::routes())
        .merge(replies::routes())
        .merge(draft_replies::routes())
        .merge(feedback::routes())
        .merge(git_http::routes())
        .merge(doctor::routes())
        .merge(events::routes())
        .merge(ui::routes())
        .fallback(reply::no_route)
        .method_not_allowed_fallback(reply::wrong_method)
        .layer(middleware::from_fn_with_state(state.clone(), guard::guard))
        .with_state(state.clone());

    let mut sigterm = signal(SignalKind::terminate()).map_err(io("handling SIGTERM"))?;
    home.write_info(&HubInfo { addr, pid })
        .map_err(io("writing hub.json"))?;
    tracing::info!(
        "hub listening on http://{addr}, data in {}",
        home.root().display()
    );

    let result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                () = state.shutdown.notified() => {}
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
            tracing::info!("hub stopping");
            state.events.stop();
        })
        .await
        .map_err(io("serving HTTP"));
    home.remove_info(pid);
    result
}
