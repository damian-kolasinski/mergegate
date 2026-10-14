//! Keeping each project's triage in step with its hand-edited rules file.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use api::event::UiEvent;
use mergegate_core::item::ItemId;
use mergegate_core::project::{ProjectName, ProjectStore};
use mergegate_core::store::StoreError;
use mergegate_core::triage::{self, RulesFile, Synced};
use rules_toml::RulesToml;
use store_sqlite::Store;

use crate::server::AppState;

/// How often the files are read for the operator's edits: each is read,
/// parsed and compared with the applied rules, well under a millisecond, and
/// an edit shows up in the UI this much later.
const POLL: Duration = Duration::from_secs(1);

pub(crate) struct RulesFiles {
    pub(super) file: RulesToml,
    /// Why each project's file can't be used, until it can.
    errors: Mutex<HashMap<ProjectName, String>>,
}

/// What a sync or a decision changed, for the UI to hear of.
#[derive(Debug, Default)]
pub(crate) struct Changes {
    /// Projects whose rules, their error, or the changes waiting changed.
    pub(super) rules: Vec<ProjectName>,
    /// Open items whose files moved.
    pub(super) items: Vec<ItemId>,
}

impl Changes {
    pub(super) fn rules(project: &ProjectName, items: Vec<ItemId>) -> Self {
        Self {
            rules: vec![project.clone()],
            items,
        }
    }

    fn extend(&mut self, other: Self) {
        self.rules.extend(other.rules);
        self.items.extend(other.items);
    }
}

impl RulesFiles {
    pub(crate) fn new(file: RulesToml) -> Self {
        Self {
            file,
            errors: Mutex::default(),
        }
    }

    /// Applies `project`'s file if it changed, or holds the edit when it
    /// would hide more.
    pub(crate) fn sync(
        &self,
        store: &mut Store,
        project: &ProjectName,
    ) -> Result<Changes, StoreError> {
        let synced = triage::sync(store, &self.file, project)?;
        let (error, items) = match synced {
            Synced::Invalid(error) => (Some(error), None),
            Synced::Unchanged => (None, None),
            Synced::Held => (None, Some(Vec::new())),
            Synced::Applied(items) => (None, Some(items)),
        };
        let mut errors = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        let error_changed = errors.get(project) != error.as_ref();
        match error {
            Some(error) => {
                if error_changed {
                    tracing::warn!("{} can't be used: {error}", self.file.location(project));
                }
                errors.insert(project.clone(), error);
            }
            None => {
                errors.remove(project);
            }
        }
        Ok(match items {
            Some(items) => Changes::rules(project, items),
            None if error_changed => Changes::rules(project, Vec::new()),
            None => Changes::default(),
        })
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

    pub(super) fn error(&self, project: &ProjectName) -> Option<String> {
        let errors = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        errors.get(project).cloned()
    }
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
        let changes = state
            .blocking(move |store, _| -> Result<Changes, StoreError> {
                let mut changes = Changes::default();
                // One project's failure leaves the others' edits applying.
                for project in store.projects()? {
                    match app.rules.sync(store, &project.name) {
                        Ok(synced) => changes.extend(synced),
                        Err(e) => app.rules.failed(&project.name, &e),
                    }
                }
                Ok(changes)
            })
            .await;
        match changes {
            Ok(changes) => changed(&state, &changes),
            Err(e) => tracing::error!("listing projects for their rules failed: {e}"),
        }
    }
}

pub(crate) fn changed(state: &AppState, changes: &Changes) {
    // The inbox counts the changes waiting for the operator.
    if !changes.rules.is_empty() || !changes.items.is_empty() {
        state.events.send(UiEvent::InboxChanged);
    }
    for project in &changes.rules {
        state.events.send(UiEvent::RulesChanged {
            project: project.to_string(),
        });
    }
    for id in &changes.items {
        state.events.send(UiEvent::ItemChanged {
            item: id.to_string(),
        });
    }
}
