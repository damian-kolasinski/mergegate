//! `/api/projects/{name}/rules/proposals`: rules an agent proposes when the
//! operator asks, and the operator's answer to a `trusted` one.

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use mergegate_core::project;
use mergegate_core::triage::{self, ChangeError, Decision, NewRule, RuleId, RulesStore};

use super::list::{self, kind_from_api, proposal_to_api, tier_from_api};
use super::watch::{Changes, changed};
use crate::reply::Failure;
use crate::server::AppState;
use crate::{items, projects};

pub(super) async fn propose(
    State(state): State<Arc<AppState>>,
    name: Result<Path<String>, PathRejection>,
    request: Result<Json<api::rules::ProposeRule>, JsonRejection>,
) -> Result<(StatusCode, Json<api::rules::Proposal>), Failure> {
    let Path(name) = name?;
    let Json(request) = request?;
    let rule = NewRule {
        tier: tier_from_api(request.tier),
        path: request.path,
        kind: request.kind.map(kind_from_api),
        max_lines: request.max_lines,
        diff_matches: request.diff_matches,
        reason: request.reason,
    };
    let app = state.clone();
    let (changes, proposed) = state
        .blocking(move |store, _| {
            let project = project::find(store, &name).map_err(projects::failure)?;
            // The hub's own sync first, so a hand edit it applies or holds
            // is heard of, and a file it can't use shows its error.
            let mut changes = app
                .rules
                .sync(store, &project.name)
                .map_err(|e| items::failure(e.into()))?;
            let proposed = triage::propose(store, &app.rules.file, &project.name, rule)
                .and_then(|proposed| {
                    let api = proposal_to_api(store, &proposed.proposal)?;
                    Ok((api, proposed.moved))
                })
                .map(|(api, moved)| {
                    changes.rules.push(project.name.clone());
                    changes.items.extend(moved);
                    api
                });
            Ok::<_, Failure>((changes, proposed))
        })
        .await?;
    changed(&state, &changes);
    Ok((StatusCode::CREATED, Json(proposed.map_err(failure)?)))
}

pub(super) async fn show(
    State(state): State<Arc<AppState>>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Json<api::rules::Proposal>, Failure> {
    let Path((name, id)) = path?;
    let id = parse(&id)?;
    let proposal = state
        .blocking(move |store, _| {
            let project = project::find(store, &name).map_err(projects::failure)?;
            let proposal = store
                .proposal(&project.name, id)
                .map_err(|e| failure(e.into()))?
                .ok_or_else(|| failure(ChangeError::NotFound(id)))?;
            proposal_to_api(store, &proposal).map_err(|e| failure(e.into()))
        })
        .await?;
    Ok(Json(proposal))
}

pub(super) async fn approve(
    state: State<Arc<AppState>>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Json<api::rules::Rules>, Failure> {
    decide(state, path, Decision::Approve).await
}

pub(super) async fn reject(
    state: State<Arc<AppState>>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Json<api::rules::Rules>, Failure> {
    decide(state, path, Decision::Reject).await
}

async fn decide(
    State(state): State<Arc<AppState>>,
    path: Result<Path<(String, String)>, PathRejection>,
    decision: Decision,
) -> Result<Json<api::rules::Rules>, Failure> {
    let Path((name, id)) = path?;
    let id = parse(&id)?;
    let app = state.clone();
    let (changes, rules) = state
        .blocking(move |store, _| {
            let project = project::find(store, &name).map_err(projects::failure)?;
            let moved =
                triage::decide_proposal(store, &app.rules.file, &project.name, id, decision)
                    .map_err(failure)?;
            let rules =
                list::read(store, &app.rules, &project.name).map_err(|e| failure(e.into()))?;
            Ok::<_, Failure>((Changes::rules(&project.name, moved), rules))
        })
        .await?;
    changed(&state, &changes);
    Ok(Json(rules))
}

fn parse(id: &str) -> Result<RuleId, Failure> {
    RuleId::parse(id).ok_or_else(|| {
        Failure::new(
            StatusCode::BAD_REQUEST,
            "invalid_rule_id",
            format!("{id:?} isn't a rule ID, as in R3"),
            None,
        )
    })
}

pub(super) fn failure(e: ChangeError) -> Failure {
    use ChangeError as E;
    use StatusCode as S;
    let (status, code, hint) = match &e {
        E::NotFound(_) => (
            S::NOT_FOUND,
            "rule_not_found",
            Some("mg rules list".to_owned()),
        ),
        E::NotPending(..) => (S::CONFLICT, "rule_decided", None),
        E::NoEdit => (S::NOT_FOUND, "no_held_edit", None),
        E::Rule(_) => (S::BAD_REQUEST, "invalid_rule", None),
        E::InvalidFile(_) => (S::CONFLICT, "rules_file_invalid", None),
        E::File(_) => (S::INTERNAL_SERVER_ERROR, "rules_file_failed", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}
