//! The hub's HTTP server.

use std::net::Ipv4Addr;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::Notify;

use crate::{Home, HubError, HubInfo, io};

struct AppState {
    health: api::Health,
    port: u16,
    shutdown: Notify,
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
        health: api::Health {
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
        .route("/api/health", get(health))
        .route("/api/hub/shutdown", post(shutdown))
        .layer(middleware::from_fn_with_state(state.clone(), guard))
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

/// Accepts only requests addressed to the loopback interface (against DNS
/// rebinding) and mutating requests that carry the client header (against
/// cross-site requests from a browser).
async fn guard(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    let port = state.port;
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        return forbidden("requests must address the hub as 127.0.0.1 or localhost");
    }
    if req.method() != Method::GET && !req.headers().contains_key(api::CLIENT_HEADER) {
        return forbidden(&format!(
            "requests must carry the {} header",
            api::CLIENT_HEADER
        ));
    }
    next.run(req).await
}

fn forbidden(message: &str) -> Response {
    let body = api::ErrorEnvelope {
        error: api::ErrorBody {
            code: "forbidden".into(),
            message: message.into(),
            hint: None,
        },
    };
    (StatusCode::FORBIDDEN, Json(body)).into_response()
}

async fn health(State(state): State<Arc<AppState>>) -> Json<api::Health> {
    Json(state.health.clone())
}

async fn shutdown(State(state): State<Arc<AppState>>) -> StatusCode {
    state.shutdown.notify_one();
    StatusCode::ACCEPTED
}
