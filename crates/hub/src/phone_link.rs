//! `/api/hub/qr`: a page of the UI as a QR code of its tailnet link, so the
//! operator picks up on a phone on the tailnet where the browser is.

use std::sync::Arc;

use axum::Router;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use qrcode::QrCode;
use qrcode::render::svg;
use serde::Deserialize;

use crate::reply::Failure;
use crate::server::AppState;
use crate::tailnet::Listening;

/// Longer routes than any the UI has would make a code too dense to scan.
const MAX_ROUTE: usize = 512;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/hub/qr", get(show))
}

#[derive(Deserialize)]
struct Page {
    /// The UI's hash route, as `#/item/3`; the inbox when absent.
    #[serde(default = "inbox")]
    route: String,
}

fn inbox() -> String {
    "#/".to_owned()
}

async fn show(
    State(state): State<Arc<AppState>>,
    query: Result<Query<Page>, QueryRejection>,
) -> Result<Response, Failure> {
    let Query(page) = query?;
    let Listening::Bound(bound) = state.tailnet.listening() else {
        return Err(Failure::new(
            StatusCode::NOT_FOUND,
            "not_on_tailnet",
            "the hub doesn't listen on the tailnet, so no phone reaches it",
            Some("mg hub set listen tailnet".to_owned()),
        ));
    };
    let link = link(&bound.url, &page.route)?;
    let svg = QrCode::new(link.as_bytes())
        .expect("a bounded route fits in a QR code")
        .render::<svg::Color>()
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    let headers = [
        (CONTENT_TYPE, "image/svg+xml"),
        (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        // The tailnet listener can move, as on `listen local` and back.
        (CACHE_CONTROL, "no-store"),
    ];
    Ok((headers, svg).into_response())
}

/// `route` on the hub at `tailnet`, as the browser on a phone opens it.
fn link(tailnet: &str, route: &str) -> Result<String, Failure> {
    let valid = route.starts_with("#/")
        && route.len() <= MAX_ROUTE
        && route.bytes().all(|b| b.is_ascii_graphic());
    if !valid {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "invalid_route",
            format!("route must be a UI route such as #/item/3, of at most {MAX_ROUTE} characters"),
            None,
        ));
    }
    Ok(format!("{}/{route}", tailnet.trim_end_matches('/')))
}

#[cfg(test)]
#[path = "phone_link_tests.rs"]
mod tests;
