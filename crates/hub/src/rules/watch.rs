//! Keeping each project's triage in step with its hand-edited rules file.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use api::event::UiEvent;
use mergegate_core::item::ItemId;
use mergegate_core::project::{ProjectName, ProjectStore};
use mergegate_core::store::StoreError;
use mergegate_core::triage::{self, RulesFile, RulesFileError, Synced};
use rules_toml::RulesToml;
use store_sqlite::Store;

use crate::server::AppState;

/// How often the files are read for the operator's edits; an edit shows up
/// in the UI this much later.
const POLL: Duration = Duration::from_secs(1);

/// Every this many polls, each file is synced whether its text changed or
/// not, as what a sync makes of the same text can change with the store:
/// a held edit approved while the file had moved on, or a rule ID the hub
/// gave out since. The UI's reads of the rules sync on their own.
const FULL: u32 = 30;

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

/// Reads every project's file each [`POLL`] with the store unlocked, and
/// syncs under its lock only the files whose text changed since their last
/// sync, so an edit re-triages open items without anyone asking.
pub(crate) async fn run(state: Arc<AppState>) {
    let mut ticks = tokio::time::interval(POLL);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut stopping = state.events.stopping();
    let mut seen = Seen::default();
    loop {
        tokio::select! {
            _ = ticks.tick() => {}
            _ = stopping.wait_for(|stop| *stop) => return,
        }
        let projects = match state.blocking(|store, _| store.projects()).await {
            Ok(projects) => projects.into_iter().map(|project| project.name),
            Err(e) => {
                tracing::error!("listing projects for their rules failed: {e}");
                continue;
            }
        };
        let projects: Vec<_> = projects.collect();
        let app = state.clone();
        let read = tokio::task::spawn_blocking(move || {
            projects
                .into_iter()
                .map(|project| {
                    let text = app.rules.file.text(&project);
                    (project, text)
                })
                .collect()
        })
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()));
        let stale = seen.stale(read);
        if stale.is_empty() {
            continue;
        }
        let app = state.clone();
        let (changes, synced) = state
            .blocking(move |store, _| {
                let mut changes = Changes::default();
                let mut synced = Vec::new();
                // One project's failure leaves the others' edits applying.
                for (project, text) in stale {
                    match app.rules.sync(store, &project) {
                        Ok(applied) => {
                            changes.extend(applied);
                            synced.push((project, text));
                        }
                        Err(e) => app.rules.failed(&project, &e),
                    }
                }
                (changes, synced)
            })
            .await;
        seen.synced(synced);
        changed(&state, &changes);
    }
}

/// A project's file as the poll read it: its text, none without a file, or
/// why it couldn't be read, which its sync reports.
type Read = Result<Option<String>, RulesFileError>;

/// The text each project's file was synced from last, so a poll syncs only
/// the files that changed since, and every one each [`FULL`] polls.
#[derive(Default)]
struct Seen {
    texts: HashMap<ProjectName, Option<String>>,
    polls: u32,
}

impl Seen {
    /// The files of `read` to sync. A file read as the text it was synced
    /// from needs none; one that couldn't be read is synced every time.
    fn stale(&mut self, read: Vec<(ProjectName, Read)>) -> Vec<(ProjectName, Read)> {
        self.polls = (self.polls + 1) % FULL;
        let full = self.polls == 0;
        self.texts
            .retain(|project, _| read.iter().any(|(listed, _)| listed == project));
        read.into_iter()
            .filter(|(project, text)| {
                full || match text {
                    Ok(text) => self.texts.get(project) != Some(text),
                    Err(_) => true,
                }
            })
            .collect()
    }

    /// The files synced, each from the text the poll read before its sync,
    /// which read it again: an edit in between is newer than the text kept,
    /// so the next poll syncs it.
    fn synced(&mut self, synced: Vec<(ProjectName, Read)>) {
        for (project, text) in synced {
            match text {
                Ok(text) => {
                    self.texts.insert(project, text);
                }
                Err(_) => {
                    self.texts.remove(&project);
                }
            }
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

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
