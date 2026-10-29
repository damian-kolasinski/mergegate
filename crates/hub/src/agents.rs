//! `/api/agents`: whether the coding agents on the hub's machine load
//! mergegate's skill stub and session hook, and setting them up, for the
//! UI's prompt. Only from the hub's own machine: a device on the tailnet
//! would set up the agents of a machine that isn't its own. Also brings up
//! to date, as the hub starts, what an older `mg` set up, so an upgrade
//! needs no `mg skill setup`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use api::agent_setup::{
    AgentId, AgentSetup, AgentsStatus, SetupChange, SetupItem, SetupKind, SetupReport, SetupState,
    SetupStep,
};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use skill::{Agent, AgentStatus, Change, Item, Kind, Scope, State, Step};

use crate::guard::Caller;
use crate::home::Home;
use crate::reply::Failure;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/agents", get(show))
        .route("/api/agents/setup", post(set_up))
}

async fn show(Extension(caller): Extension<Caller>) -> Result<Json<AgentsStatus>, Failure> {
    let scope = local(&caller)?;
    let status = tokio::task::spawn_blocking(move || skill::status(&scope))
        .await
        .map_err(failed)?;
    Ok(Json(to_api(&status)))
}

/// Sets up the agents found on the machine, as `mg skill setup` does.
async fn set_up(Extension(caller): Extension<Caller>) -> Result<Json<SetupReport>, Failure> {
    let scope = local(&caller)?;
    let steps = tokio::task::spawn_blocking(move || {
        let steps = skill::setup(&scope, &scope.found(), false, false);
        log(&steps);
        steps
    })
    .await
    .map_err(failed)?;
    Ok(Json(report(false, &steps)))
}

/// The user's agents, for a caller on this machine.
fn local(caller: &Caller) -> Result<Scope, Failure> {
    if !matches!(caller, Caller::Local) {
        return Err(Failure::new(
            StatusCode::FORBIDDEN,
            "local_only",
            "only the hub's own machine sets up its agents",
            Some("run mg skill setup on the machine whose agents to set up".into()),
        ));
    }
    user_scope().ok_or_else(|| {
        Failure::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "no_home",
            "HOME isn't set for the hub, so it can't find the agents",
            None,
        )
    })
}

fn failed(e: tokio::task::JoinError) -> Failure {
    Failure::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "agents_failed",
        e.to_string(),
        None,
    )
}

/// The agents of the user `HOME` names.
pub fn user_scope() -> Option<Scope> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| Scope::user(PathBuf::from(home)))
}

/// Run as the hub starts. Only for the data directory an installed `mg`
/// uses: a dev or test hub's `mg` is not the one agents run.
pub(crate) async fn refresh(home: PathBuf) {
    if !is_default(&home) {
        return;
    }
    let Some(scope) = user_scope() else { return };
    match tokio::task::spawn_blocking(move || skill::refresh(&scope)).await {
        Ok(steps) => log(&steps),
        Err(e) => tracing::warn!("refreshing the agents' skill: {e}"),
    }
}

fn is_default(root: &Path) -> bool {
    let canonical = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    Home::default_root().is_ok_and(|default| canonical(&default) == canonical(root))
}

fn log(steps: &[Step]) {
    for step in steps {
        let path = step.path.as_deref().map(Path::display);
        match &step.change {
            Change::Failed(why) => tracing::warn!("agent setup: {path:?} failed: {why}"),
            Change::Installed | Change::Updated | Change::Removed => {
                tracing::info!("agent setup: {:?} {path:?}", step.change);
            }
            _ => {}
        }
    }
}

pub fn to_api(status: &[AgentStatus]) -> AgentsStatus {
    AgentsStatus {
        agents: status
            .iter()
            .map(|agent| AgentSetup {
                agent: agent_id(agent.agent),
                label: agent.agent.label().to_owned(),
                found: agent.found,
                ready: agent.ready(),
                skill: item(&agent.skill),
                hook: item(&agent.hook),
            })
            .collect(),
    }
}

pub fn report(dry_run: bool, steps: &[Step]) -> SetupReport {
    SetupReport {
        dry_run,
        steps: steps
            .iter()
            .map(|step| SetupStep {
                agents: step.agents.iter().copied().map(agent_id).collect(),
                kind: match step.kind {
                    Kind::Skill => SetupKind::Skill,
                    Kind::Hook => SetupKind::Hook,
                },
                path: step.path.as_ref().map(|p| p.display().to_string()),
                change: match &step.change {
                    Change::Installed => SetupChange::Installed,
                    Change::Updated => SetupChange::Updated,
                    Change::Unchanged => SetupChange::Unchanged,
                    Change::Removed => SetupChange::Removed,
                    Change::Absent => SetupChange::Absent,
                    Change::Conflict => SetupChange::Conflict,
                    Change::Kept => SetupChange::Kept,
                    Change::Unsupported => SetupChange::Unsupported,
                    Change::Failed(_) => SetupChange::Failed,
                },
                reason: match &step.change {
                    Change::Failed(why) => Some(why.clone()),
                    _ => None,
                },
                approve: step.approval().map(str::to_owned),
            })
            .collect(),
    }
}

pub fn agent_id(agent: Agent) -> AgentId {
    match agent {
        Agent::Claude => AgentId::Claude,
        Agent::Codex => AgentId::Codex,
        Agent::OpenCode => AgentId::OpenCode,
    }
}

fn item(item: &Item) -> SetupItem {
    let (state, reason) = match &item.state {
        State::Current => (SetupState::Current, None),
        State::Outdated => (SetupState::Outdated, None),
        State::Missing => (SetupState::Missing, None),
        State::Foreign => (SetupState::Foreign, None),
        State::Unreadable(why) => (SetupState::Unreadable, Some(why.clone())),
        State::Unsupported => (SetupState::Unsupported, None),
    };
    SetupItem {
        path: item.path.as_ref().map(|p| p.display().to_string()),
        state,
        reason,
    }
}
