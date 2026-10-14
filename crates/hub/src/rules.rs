//! `/api/projects/{name}/rules`, and keeping each project's triage in step
//! with its hand-edited rules file.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use api::event::UiEvent;
use axum::extract::rejection::PathRejection;
use axum::extract::{self, State};
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::item::ItemId;
use mergegate_core::project::{self, ProjectName, ProjectStore};
use mergegate_core::store::StoreError;
use mergegate_core::triage::{self, Kind, Rule, RulesFile, RulesStore, Synced, Tier};
use rules_toml::RulesToml;
use store_sqlite::Store;

use crate::reply::Failure;
use crate::server::AppState;
use crate::{items, projects};

/// How often the files are read for the operator's edits: each is read,
/// parsed and compared with the applied rules, well under a millisecond, and
/// an edit shows up in the UI this much later.
const POLL: Duration = Duration::from_secs(1);

pub(crate) struct RulesFiles {
    file: RulesToml,
    /// Why each project's file can't be used, until it can.
    errors: Mutex<HashMap<ProjectName, String>>,
}

impl RulesFiles {
    pub(crate) fn new(file: RulesToml) -> Self {
        Self {
            file,
            errors: Mutex::default(),
        }
    }

    /// Applies `project`'s file if it changed, returning the open items whose
    /// files moved.
    pub(crate) fn sync(
        &self,
        store: &mut Store,
        project: &ProjectName,
    ) -> Result<Vec<ItemId>, StoreError> {
        let synced = triage::sync(store, &self.file, project)?;
        let mut errors = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        match synced {
            Synced::Invalid(error) => {
                if errors.get(project) != Some(&error) {
                    tracing::warn!("{} can't be used: {error}", self.file.location(project));
                }
                errors.insert(project.clone(), error);
                Ok(Vec::new())
            }
            Synced::Unchanged => {
                errors.remove(project);
                Ok(Vec::new())
            }
            Synced::Applied(items) => {
                errors.remove(project);
                Ok(items)
            }
        }
    }

    /// Logs a project's failed sync once per distinct error, as the poll
    /// retries it every second.
    fn failed(&self, project: &ProjectName, error: &StoreError) {
        let error = format!("applying the rules failed: {error}");
        let mut errors = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        if errors.get(project) != Some(&error) {
            tracing::error!("{project}: {error}");
        }
        errors.insert(project.clone(), error);
    }

    fn error(&self, project: &ProjectName) -> Option<String> {
        let errors = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        errors.get(project).cloned()
    }
}

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/projects/{name}/rules", get(list))
}

async fn list(
    State(state): State<Arc<AppState>>,
    name: Result<extract::Path<String>, PathRejection>,
) -> Result<Json<api::rules::Rules>, Failure> {
    let extract::Path(name) = name?;
    let app = state.clone();
    let (project, rules, moved) = state
        .blocking(move |store, _| {
            let project = project::find(store, &name).map_err(projects::failure)?;
            let moved = app
                .rules
                .sync(store, &project.name)
                .map_err(|e| items::failure(e.into()))?;
            let rules = store
                .applied_rules(&project.name)
                .map_err(|e| items::failure(e.into()))?;
            Ok::<_, Failure>((project.name, rules, moved))
        })
        .await?;
    changed(&state, &moved);
    Ok(Json(api::rules::Rules {
        location: state.rules.file.location(&project),
        error: state.rules.error(&project),
        rules: rules.rules().map(rule_to_api).collect(),
        project: project.to_string(),
    }))
}

/// Re-reads every project's file each [`POLL`], so an edit re-triages open
/// items without anyone asking.
pub(crate) async fn run(state: Arc<AppState>) {
    let mut ticks = tokio::time::interval(POLL);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut stopping = state.events.stopping();
    loop {
        tokio::select! {
            _ = ticks.tick() => {}
            _ = stopping.wait_for(|stop| *stop) => return,
        }
        let app = state.clone();
        let moved = state
            .blocking(move |store, _| -> Result<Vec<ItemId>, StoreError> {
                let mut moved = Vec::new();
                // One project's failure leaves the others' edits applying.
                for project in store.projects()? {
                    match app.rules.sync(store, &project.name) {
                        Ok(items) => moved.extend(items),
                        Err(e) => app.rules.failed(&project.name, &e),
                    }
                }
                Ok(moved)
            })
            .await;
        match moved {
            Ok(moved) => changed(&state, &moved),
            Err(e) => tracing::error!("listing projects for their rules failed: {e}"),
        }
    }
}

pub(crate) fn changed(state: &AppState, moved: &[ItemId]) {
    if moved.is_empty() {
        return;
    }
    state.events.send(UiEvent::InboxChanged);
    for id in moved {
        state.events.send(UiEvent::ItemChanged {
            item: id.to_string(),
        });
    }
}

fn rule_to_api(rule: &Rule) -> api::rules::Rule {
    api::rules::Rule {
        id: rule.id.to_string(),
        tier: tier_to_api(rule.tier),
        path: rule.path.clone(),
        kind: rule.kind.map(kind_to_api),
        max_lines: rule.max_lines,
        diff_matches: rule.diff_matches.clone(),
        reason: rule.reason.clone(),
        summary: rule.summary(),
    }
}

pub(crate) fn tier_to_api(tier: Tier) -> api::rules::Tier {
    match tier {
        Tier::Review => api::rules::Tier::Review,
        Tier::Trusted => api::rules::Tier::Trusted,
    }
}

fn kind_to_api(kind: Kind) -> api::rules::Kind {
    match kind {
        Kind::Added => api::rules::Kind::Added,
        Kind::Deleted => api::rules::Kind::Deleted,
        Kind::Renamed => api::rules::Kind::Renamed,
        Kind::Modified => api::rules::Kind::Modified,
        Kind::Generated => api::rules::Kind::Generated,
    }
}
