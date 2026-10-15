//! `POST /api/projects/{name}/rules/edit/approve` and `.../reject`: the
//! operator's answer to a hand edit held because it would hide more.

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use mergegate_core::project;
use mergegate_core::triage::{self, Decision};

use super::list;
use super::proposals::failure;
use super::watch::{Changes, changed};
use crate::projects;
use crate::reply::Failure;
use crate::server::AppState;

pub(super) async fn approve(
    state: State<Arc<AppState>>,
    name: Result<Path<String>, PathRejection>,
) -> Result<Json<api::rules::Rules>, Failure> {
    decide(state, name, Decision::Approve).await
}

pub(super) async fn reject(
    state: State<Arc<AppState>>,
    name: Result<Path<String>, PathRejection>,
) -> Result<Json<api::rules::Rules>, Failure> {
    decide(state, name, Decision::Reject).await
}

async fn decide(
    State(state): State<Arc<AppState>>,
    name: Result<Path<String>, PathRejection>,
    decision: Decision,
) -> Result<Json<api::rules::Rules>, Failure> {
    let Path(name) = name?;
    let app = state.clone();
    let (changes, rules) = state
        .blocking(move |store, _| {
            let project = project::find(store, &name).map_err(projects::failure)?;
            let moved = triage::decide_edit(store, &app.rules.file, &project.name, decision)
                .map_err(failure)?;
            let rules =
                list::read(store, &app.rules, &project.name).map_err(|e| failure(e.into()))?;
            Ok::<_, Failure>((Changes::rules(&project.name, moved), rules))
        })
        .await?;
    changed(&state, &changes);
    Ok(Json(rules))
}
