//! `GET /api/whoami`, and `/api/hub/devices`: the tailnet devices and the
//! operator's decisions about them, which `mg hub devices`, `allow` and
//! `deny` show and change.

use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Extension, Json, Router};
use mergegate_core::device::{
    self, Access, DeviceError, DeviceStore, Listed, Standing, TailnetError, Trust,
};

use crate::guard::Caller;
use crate::reply::Failure;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/whoami", get(whoami))
        .route("/api/hub/devices", get(devices))
        .route("/api/hub/devices/{name}", put(decide))
}

async fn whoami(Extension(caller): Extension<Caller>) -> Json<api::device::WhoAmI> {
    Json(match caller {
        Caller::Local => api::device::WhoAmI {
            via: api::device::Via::Loopback,
            device: None,
        },
        Caller::Device(device, trust, _) => api::device::WhoAmI {
            via: api::device::Via::Tailnet,
            device: Some(api::device::Caller {
                name: device.name,
                owner: device.owner.describe(),
                trust: match trust {
                    Trust::Owner => api::device::Trust::Owner,
                    Trust::Allowed => api::device::Trust::Allowed,
                },
            }),
        },
    })
}

async fn devices(
    State(state): State<Arc<AppState>>,
) -> Result<Json<api::device::Devices>, Failure> {
    // Asked before taking the store, which other requests wait on.
    let status = state
        .tailnet
        .status()
        .await
        .map_err(|e| failure(e.into()))?;
    let records = state
        .blocking(|store, _| store.devices())
        .await
        .map_err(|e| failure(e.into()))?;
    let devices = device::list(&status, &records)
        .into_iter()
        .map(to_api)
        .collect();
    Ok(Json(api::device::Devices { devices }))
}

async fn decide(
    State(state): State<Arc<AppState>>,
    name: Result<Path<String>, PathRejection>,
    request: Result<Json<api::device::SetAccess>, JsonRejection>,
) -> Result<Json<api::device::Decided>, Failure> {
    let Path(name) = name?;
    let Json(request) = request?;
    let access = match request.access {
        api::device::Access::Allow => Access::Allow,
        api::device::Access::Deny => Access::Deny,
    };
    let tailscale = state.tailnet.tailscale();
    let record = state
        .blocking(move |store, _| device::decide(store, &*tailscale, &name, access))
        .await
        .map_err(failure)?;
    // A denied device's open `mg wait` or UI stream ends now, not at its
    // next request.
    state.tailnet.readmit();
    tracing::info!(
        event_code = "device_decided",
        "{:?} {} ({})",
        access,
        record.name,
        record.owner
    );
    Ok(Json(api::device::Decided {
        name: record.name,
        owner: record.owner,
        access: request.access,
    }))
}

fn to_api(listed: Listed) -> api::device::Device {
    use api::device::Standing as S;
    api::device::Device {
        name: listed.name,
        owner: listed.owner,
        standing: match listed.standing {
            Standing::Owner => S::Owner,
            Standing::Allowed => S::Allowed,
            Standing::Denied => S::Denied,
            Standing::Asked => S::Asked,
            Standing::NotAllowed => S::NotAllowed,
        },
        on_tailnet: listed.on_tailnet,
    }
}

fn failure(e: DeviceError) -> Failure {
    use StatusCode as S;
    let (status, code) = match &e {
        DeviceError::Unknown { .. } => (S::NOT_FOUND, "unknown_device"),
        DeviceError::Ambiguous { .. } => (S::CONFLICT, "ambiguous_device"),
        DeviceError::Tailnet(TailnetError::Unknown(_)) => (S::NOT_FOUND, "unknown_device"),
        DeviceError::Tailnet(TailnetError::Unavailable(_)) => {
            (S::SERVICE_UNAVAILABLE, "tailscale_unavailable")
        }
        DeviceError::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed"),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    let hint = match code {
        "unknown_device" => Some("mg hub devices".to_owned()),
        "ambiguous_device" => Some("name the device by its full name or address".to_owned()),
        _ => None,
    };
    Failure::new(status, code, e.to_string(), hint)
}
