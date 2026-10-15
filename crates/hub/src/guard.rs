use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use mergegate_core::device::{self, Device, Refusal, TailnetError, Trust};

use crate::git_http;
use crate::reply::Failure;
use crate::server::AppState;
use crate::tailnet::{Bound, Via};

/// Who made a request, for the routes that treat callers differently.
#[derive(Clone)]
pub(crate) enum Caller {
    /// On the hub's machine, through loopback.
    Local,
    Device(Device, Trust),
}

/// Accepts only requests addressed to the listener they came in on (against
/// DNS rebinding), from a trusted device when that is the tailnet, and
/// mutating requests that carry the client header or come from git (against
/// cross-site requests from a browser).
pub(crate) async fn guard(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let caller = match req.extensions().get::<Via>().cloned() {
        Some(Via::Tailnet(bound)) => {
            let peer = req
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|c| c.0);
            match admit(&state, &bound, &host, peer).await {
                Ok(caller) => caller,
                Err(refused) => return refused.into_response(),
            }
        }
        Some(Via::Loopback) | None => {
            let port = state.port;
            if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
                return forbidden("requests must address the hub as 127.0.0.1 or localhost");
            }
            Caller::Local
        }
    };
    if req.method() != Method::GET
        && !req.headers().contains_key(api::CLIENT_HEADER)
        && !from_git(&req)
    {
        return forbidden(&format!(
            "requests must carry the {} header",
            api::CLIENT_HEADER
        ));
    }
    req.extensions_mut().insert(caller);
    next.run(req).await
}

async fn admit(
    state: &Arc<AppState>,
    bound: &Arc<Bound>,
    host: &str,
    peer: Option<SocketAddr>,
) -> Result<Caller, Failure> {
    if !state.tailnet.is_current(bound) {
        return Err(Failure::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "the hub no longer listens on the tailnet",
            None,
        ));
    }
    if !bound.addressed_as(host) {
        return Err(Failure::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            format!(
                "requests over the tailnet must address the hub as {}",
                bound.url
            ),
            None,
        ));
    }
    let Some(peer) = peer else {
        return Err(unknown("no address"));
    };
    let caller = match state.tailnet.whois(peer.ip()).await {
        Ok(caller) => caller,
        Err(TailnetError::Unknown(ip)) => return Err(unknown(&ip.to_string())),
        Err(e @ TailnetError::Unavailable(_)) => {
            tracing::warn!("can't tell who {peer} is: {e}");
            return Err(Failure::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "tailscale_unavailable",
                format!("the hub can't ask Tailscale who is calling: {e}"),
                None,
            ));
        }
    };
    let me = bound.me.clone();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let asking = caller.clone();
    let decision = state
        .blocking(move |store, _| device::admit(store, &me, &asking, now))
        .await
        .map_err(|e| {
            tracing::error!("{e}");
            Failure::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_failed",
                e.to_string(),
                None,
            )
        })?;
    match decision {
        Ok(trust) => Ok(Caller::Device(caller, trust)),
        Err(refusal) => {
            let (code, why) = match refusal {
                Refusal::Denied => ("device_denied", "the operator denied it"),
                Refusal::NotAllowed => (
                    "device_not_allowed",
                    "it isn't the hub owner's and the operator hasn't allowed it",
                ),
            };
            tracing::info!(
                "refused {} ({}): {code}",
                caller.name,
                caller.owner.describe()
            );
            Err(Failure::new(
                StatusCode::FORBIDDEN,
                code,
                format!(
                    "the hub refuses {} ({}): {why}",
                    caller.name,
                    caller.owner.describe()
                ),
                Some(format!("on the hub: mg hub allow {}", caller.name)),
            ))
        }
    }
}

fn unknown(address: &str) -> Failure {
    Failure::new(
        StatusCode::FORBIDDEN,
        "unknown_caller",
        format!("no device on the tailnet has the address {address}"),
        None,
    )
}

/// A `git fetch` or `git push` to the hub's remote, which can't carry the
/// client header but sends a body type browsers can't send cross-site.
fn from_git(req: &Request) -> bool {
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    req.uri().path().starts_with("/git/") && git_http::REQUEST_TYPES.contains(&content_type)
}

fn forbidden(message: &str) -> Response {
    Failure::new(StatusCode::FORBIDDEN, "forbidden", message, None).into_response()
}
