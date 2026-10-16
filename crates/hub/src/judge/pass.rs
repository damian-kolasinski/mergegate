//! The background judge pass. It runs at start, after a submit and after a
//! judge change; a provider is only ever called from here, off the store.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use api::event::UiEvent;
use judge_command::CommandJudge;
use judge_jev::Jev;
use mergegate_core::item::{ItemId, ItemStore};
use mergegate_core::judge::{self, Judge, JudgeError, JudgeStore, Provider, Request, Unjudged};
use mergegate_core::project::{ProjectName, ProjectStore};
use mergegate_core::store::StoreError;

use crate::server::AppState;

/// Revisions a judge failed on, so the pass doesn't call a failing judge
/// again each time it wakes. Kept in memory: setting the judge again or
/// restarting the hub retries them.
#[derive(Default)]
pub(crate) struct Failures(Mutex<HashMap<(ItemId, u32), Failed>>);

struct Failed {
    /// The [`judge::JudgeSetting::key`] of the judge that failed.
    key: String,
    error: String,
}

impl Failures {
    fn failed(&self, unjudged: &Unjudged) -> bool {
        let failures = self.0.lock().unwrap_or_else(|e| e.into_inner());
        failures
            .get(&(unjudged.item.clone(), unjudged.revision))
            .is_some_and(|f| f.key == unjudged.setting.key())
    }

    fn insert(&self, unjudged: &Unjudged, error: String) {
        let mut failures = self.0.lock().unwrap_or_else(|e| e.into_inner());
        failures.insert(
            (unjudged.item.clone(), unjudged.revision),
            Failed {
                key: unjudged.setting.key(),
                error,
            },
        );
    }

    /// `project`'s failures under the judge `key`, oldest item first.
    pub(super) fn of(
        &self,
        store: &impl ItemStore,
        project: &ProjectName,
        key: &str,
    ) -> Result<Vec<api::judge::Failure>, StoreError> {
        let mut of: Vec<_> = {
            let failures = self.0.lock().unwrap_or_else(|e| e.into_inner());
            failures
                .iter()
                .filter(|((item, _), f)| item.project() == project && f.key == key)
                .map(|((item, revision), f)| (item.clone(), *revision, f.error.clone()))
                .collect()
        };
        of.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
        of.into_iter()
            .map(|(item, revision, error)| {
                let name = store
                    .item(&item)?
                    .map_or_else(|| item.clone(), |found| found.name());
                Ok(api::judge::Failure {
                    item: item.to_string(),
                    name: name.to_string(),
                    revision,
                    error,
                })
            })
            .collect()
    }

    pub(super) fn clear(&self, project: &ProjectName) {
        let mut failures = self.0.lock().unwrap_or_else(|e| e.into_inner());
        failures.retain(|(item, _), _| item.project() != project);
    }
}

/// Passes at start, then each time something wakes it.
pub(crate) async fn run(state: Arc<AppState>) {
    loop {
        pass(&state).await;
        state.judge.notified().await;
    }
}

async fn pass(state: &Arc<AppState>) {
    let unjudged = match state
        .blocking(|store, _| judge::unjudged(store, None))
        .await
    {
        Ok(unjudged) => unjudged,
        Err(e) => return tracing::error!("listing revisions to judge: {e}"),
    };
    for unjudged in unjudged {
        if state.stopping.load(Ordering::Relaxed) {
            return;
        }
        if state.judge_failures.failed(&unjudged) {
            continue;
        }
        let found = {
            let (item, revision) = (unjudged.item.clone(), unjudged.revision);
            state
                .blocking(move |store, _| read(store, &item, revision))
                .await
        };
        let (request, repo) = match found {
            Ok(Some(found)) => found,
            // Closed, revised or gone since the listing; a call can cost.
            Ok(None) => continue,
            Err(e) => return tracing::error!("reading {} to judge: {e}", unjudged.item),
        };
        // A provider can take minutes, so it runs without the store.
        let provider = unjudged.setting.provider.clone();
        let stop = state.stopping.clone();
        let judged = tokio::task::spawn_blocking(move || {
            let verdicts = call(&provider, repo, &request, stop);
            (request, verdicts)
        })
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()));
        let verdicts = match judged {
            (request, Ok(verdicts)) => judge::accept(&request, verdicts),
            // Not the judge's failure, and nothing is left to retry it.
            (_, Err(_)) if state.stopping.load(Ordering::Relaxed) => return,
            (_, Err(e)) => {
                tracing::warn!(
                    "judging {} revision {}: {e}",
                    unjudged.item,
                    unjudged.revision
                );
                state.judge_failures.insert(&unjudged, e.0);
                continue;
            }
        };
        let stored = {
            let unjudged = unjudged.clone();
            state
                .blocking(move |store, _| {
                    store.insert_verdicts(
                        &unjudged.item,
                        unjudged.revision,
                        &unjudged.setting.key(),
                        &verdicts,
                    )
                })
                .await
        };
        if let Err(e) = stored {
            return tracing::error!("storing verdicts: {e}");
        }
        // A promotion moves a file into review, which the inbox counts.
        state.events.send(UiEvent::InboxChanged);
        state.events.send(UiEvent::ItemChanged {
            item: unjudged.item.to_string(),
        });
    }
}

/// The request, and the project's repo a command judge runs in.
fn read(
    store: &store_sqlite::Store,
    item: &ItemId,
    revision: u32,
) -> Result<Option<(Request, PathBuf)>, StoreError> {
    let Some(project) = store.project(item.project())? else {
        return Ok(None);
    };
    Ok(judge::request(store, item, revision)?.map(|request| (request, project.upstream)))
}

fn call(
    provider: &Provider,
    repo: PathBuf,
    request: &Request,
    stop: Arc<AtomicBool>,
) -> Result<Vec<judge::Verdict>, JudgeError> {
    match provider {
        Provider::Jev { model } => Jev::from_env(model.clone())
            .stopping_on(stop)
            .judge(request),
        Provider::Command { command } => CommandJudge::from_env(command.clone(), repo)
            .stopping_on(stop)
            .judge(request),
    }
}
