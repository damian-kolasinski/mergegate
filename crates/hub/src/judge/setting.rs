//! `GET` and `PUT /api/projects/{name}/judge`: what `mg judge show` and
//! `mg judge set` read and change.

use std::sync::Arc;

use api::event::UiEvent;
use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use mergegate_core::item::ItemStore;
use mergegate_core::judge::{self, JudgeSetting, JudgeStore, Provider, SettingError};
use mergegate_core::project::{self, ProjectName};
use mergegate_core::store::StoreError;
use store_sqlite::Store;

use super::Failures;
use crate::projects;
use crate::reply::Failure;
use crate::server::AppState;

pub(super) async fn show(
    State(state): State<Arc<AppState>>,
    name: Result<Path<String>, PathRejection>,
) -> Result<Json<api::judge::Judge>, Failure> {
    let Path(name) = name?;
    let app = state.clone();
    let judge = state
        .blocking(move |store, _| {
            let project = project::find(store, &name).map_err(projects::failure)?;
            read(store, &app.judge_failures, &project.name).map_err(store_failure)
        })
        .await?;
    Ok(Json(judge))
}

pub(super) async fn set(
    State(state): State<Arc<AppState>>,
    name: Result<Path<String>, PathRejection>,
    request: Result<Json<api::judge::SetJudge>, JsonRejection>,
) -> Result<Json<api::judge::Judge>, Failure> {
    let Path(name) = name?;
    let Json(request) = request?;
    let setting = JudgeSetting::parse(
        &request.provider,
        request.model.as_deref(),
        request.command.as_deref(),
        request.threshold,
    )
    .map_err(failure)?;
    let app = state.clone();
    let (judge, open) = state
        .blocking(move |store, _| {
            let project = project::find(store, &name).map_err(projects::failure)?;
            judge::set(store, &project.name, setting.as_ref()).map_err(failure)?;
            // Setting a judge again is how the operator retries failures.
            app.judge_failures.clear(&project.name);
            let open = store
                .open_items(Some(&project.name))
                .map_err(store_failure)?;
            let judge = read(store, &app.judge_failures, &project.name).map_err(store_failure)?;
            Ok::<_, Failure>((judge, open))
        })
        .await?;
    state.judge.notify_one();
    // Another judge or threshold moves files in and out of review now,
    // before the pass has judged anything.
    state.events.send(UiEvent::InboxChanged);
    for item in open {
        state.events.send(UiEvent::ItemChanged {
            item: item.id.to_string(),
        });
    }
    Ok(Json(judge))
}

fn read(
    store: &Store,
    failures: &Failures,
    project: &ProjectName,
) -> Result<api::judge::Judge, StoreError> {
    let Some(setting) = store.judge_setting(project)? else {
        return Ok(api::judge::Judge {
            project: project.to_string(),
            provider: "none".into(),
            model: None,
            command: None,
            threshold: None,
            problem: None,
            waiting: 0,
            failures: Vec::new(),
        });
    };
    let (model, command, problem) = match &setting.provider {
        Provider::Jev { model } => (Some(model.clone()), None, judge_jev::missing_key()),
        Provider::Command { command } => (None, Some(command.clone()), None),
    };
    Ok(api::judge::Judge {
        project: project.to_string(),
        provider: setting.provider.name().into(),
        model,
        command,
        threshold: Some(setting.threshold),
        problem,
        waiting: judge::unjudged(store, Some(project))?.len() as u32,
        failures: failures.of(project, &setting.key()),
    })
}

fn failure(e: SettingError) -> Failure {
    use SettingError as E;
    let hint = match &e {
        E::NoProject(_) => {
            return Failure::new(
                StatusCode::NOT_FOUND,
                "project_not_found",
                e.to_string(),
                Some("mg project show".into()),
            );
        }
        E::Store(_) => {
            tracing::error!("{e}");
            return Failure::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_failed",
                e.to_string(),
                None,
            );
        }
        E::NoCommand => "mg judge set command --command 'CMD'",
        E::UnknownProvider(_)
        | E::Misplaced(..)
        | E::OptionsForNone
        | E::Empty(_)
        | E::Threshold(_) => "mg judge set none|jev|command",
    };
    Failure::new(
        StatusCode::BAD_REQUEST,
        "invalid_judge",
        e.to_string(),
        Some(hint.into()),
    )
}

fn store_failure(e: StoreError) -> Failure {
    failure(SettingError::Store(e))
}
