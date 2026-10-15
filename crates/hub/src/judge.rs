//! `/api/projects/{name}/judge`: each project's judge, and the background
//! pass that has it score open items' newest revisions (design §7.4).

mod pass;
mod setting;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;

use crate::server::AppState;

pub(crate) use pass::{Failures, run};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/projects/{name}/judge",
        get(setting::show).put(setting::set),
    )
}
