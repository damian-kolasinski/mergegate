//! Early conflict detection's loop: every project's open items
//! test-merged into their targets on every submit, after each of the hub's
//! merges, and on a poll for targets moved outside the hub.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use api::event::UiEvent;
use mergegate_core::conflict::{Seen, Watch};
use mergegate_core::project::{ProjectName, ProjectStore};
use mergegate_core::store::Shared;

use crate::server::AppState;

/// A fetch from the operator's repo is cheap, and one from GitHub costs no
/// API request.
const POLL_MS: u64 = 60_000;

const READING: &str = "reading open items to test-merge";
const INTO_TARGETS: &str = "test-merging open items into their targets";

pub(crate) async fn run(state: Arc<AppState>) {
    // For tests of what a merge finds, which the watch could find first.
    if std::env::var_os("MG_TARGET_WATCH").is_some_and(|v| v == "off") {
        return;
    }
    let poll = Duration::from_millis(env_ms("MG_TARGET_POLL_MS").unwrap_or(POLL_MS));
    let mut stopping = state.events.stopping();
    let mut seen: HashMap<ProjectName, Seen> = HashMap::new();
    // The last failure per project and check, so one that persists is
    // logged once.
    let mut failing: HashMap<(ProjectName, &'static str), String> = HashMap::new();
    loop {
        pass(&state, &mut seen, &mut failing).await;
        // A wake during a pass is kept, so what it asked for gets a pass.
        tokio::select! {
            () = tokio::time::sleep(poll) => {}
            () = state.conflicts.notified() => {}
            _ = stopping.wait_for(|stopping| *stopping) => return,
        }
    }
}

async fn pass(
    state: &Arc<AppState>,
    seen: &mut HashMap<ProjectName, Seen>,
    failing: &mut HashMap<(ProjectName, &'static str), String>,
) {
    let projects = match state.blocking(|store, _| store.projects()).await {
        Ok(projects) => projects,
        Err(e) => return tracing::error!("listing projects to test-merge: {e}"),
    };
    let names: Vec<ProjectName> = projects.into_iter().map(|p| p.name).collect();
    seen.retain(|name, _| names.contains(name));
    failing.retain(|(name, _), _| names.contains(name));
    for name in names {
        let tried = seen.remove(&name).unwrap_or_default();
        let project = name.clone();
        let read = state
            .blocking(move |store, _| Watch::read(store, &project))
            .await;
        let watch = match read {
            Ok(Some(watch)) => watch,
            // Removed since it was listed.
            Ok(None) => continue,
            Err(e) => {
                report(failing, &name, READING, vec![e.to_string()]);
                seen.insert(name, tried);
                continue;
            }
        };
        report(failing, &name, READING, Vec::new());
        // Fetches and merges run without the store, so a slow upstream never
        // holds up the hub's requests; with nothing moved, they merge nothing.
        let mut merges = state
            .unlocked(move |git, github| watch.try_merges(git, github, &tried))
            .await;
        let mut errors: Vec<String> = std::mem::take(&mut merges.errors)
            .iter()
            .map(ToString::to_string)
            .collect();
        let next_seen = std::mem::take(&mut merges.seen);
        // Under landing: it moves staging's target and hands items back,
        // which a merge does too.
        let handed_back = state
            .stepwise(move |steps, git, _| steps.with(|store| merges.hand_back(store, git)))
            .await;
        // A hand-back that failed partway may have notified some items, so
        // the clients refresh anyway, and the next pass tries every item.
        let (handed_back, failed) = match handed_back {
            Ok(ids) => {
                seen.insert(name.clone(), next_seen);
                (ids, false)
            }
            Err(e) => {
                errors.push(e.to_string());
                (Vec::new(), true)
            }
        };
        report(failing, &name, INTO_TARGETS, errors);
        if failed || !handed_back.is_empty() {
            state.events.logged();
            for id in &handed_back {
                state.events.send(UiEvent::ItemChanged {
                    item: id.to_string(),
                });
            }
            state.events.send(UiEvent::InboxChanged);
        }
    }
}

/// Logs what a check of a project failed on, unless it failed the same way
/// last pass. No errors clears it.
fn report(
    failing: &mut HashMap<(ProjectName, &'static str), String>,
    project: &ProjectName,
    check: &'static str,
    errors: Vec<String>,
) {
    let key = (project.clone(), check);
    if errors.is_empty() {
        failing.remove(&key);
        return;
    }
    let message = errors.join("; ");
    if failing.get(&key) != Some(&message) {
        tracing::warn!("{check} of {project}: {message}");
        failing.insert(key, message);
    }
}

fn env_ms(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}
