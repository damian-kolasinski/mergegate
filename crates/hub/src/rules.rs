//! `/api/projects/{name}/rules`: the rules each project's files are triaged
//! by, kept in step with its hand-edited file, and the changes to them that
//! wait for the operator.

mod edit;
mod list;
mod proposals;
mod watch;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};

use crate::server::AppState;

pub(crate) use list::{tier_to_api, waiting};
pub(crate) use watch::{Changes, RulesFiles, changed, run};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/projects/{name}/rules", get(list::list))
        .route(
            "/api/projects/{name}/rules/proposals",
            post(proposals::propose),
        )
        .route(
            "/api/projects/{name}/rules/proposals/{id}",
            get(proposals::show),
        )
        .route(
            "/api/projects/{name}/rules/proposals/{id}/approve",
            post(proposals::approve),
        )
        .route(
            "/api/projects/{name}/rules/proposals/{id}/reject",
            post(proposals::reject),
        )
        .route(
            "/api/projects/{name}/rules/edit/approve",
            post(edit::approve),
        )
        .route("/api/projects/{name}/rules/edit/reject", post(edit::reject))
}
