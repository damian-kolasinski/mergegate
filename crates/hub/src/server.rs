use std::net::Ipv4Addr;
use std::sync::Arc;

use axum::Router;
use axum::middleware;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::Notify;

use crate::{Home, HubError, HubInfo, guard, io, lifecycle};

pub(crate) struct AppState {
    pub health: api::hub::Health,
    pub port: u16,
    pub shutdown: Notify,
}

pub(crate) async fn serve(home: &Home, port: u16, schema_version: u32) -> Result<(), HubError> {
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
    });
    let app = Router::new()
        .merge(lifecycle::routes())
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
        })
        .await
        .map_err(io("serving HTTP"));
    home.remove_info(pid);
    result
}
