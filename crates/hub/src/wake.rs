//! Runs an item's wake command when it has events from the operator its agent
//! hasn't read, so a harness whose session ended can start one. The hub runs
//! the commands left from its own machine; another device's `mg wakes`
//! runner takes its own from [`crate::remote_wakes`] and runs them there.
//!
//! Each event wakes at most once, even across restarts: the store keeps the
//! newest cursor a command ran for. A command doesn't run while a read of
//! the item is in flight, since that read hands the agent the events, nor
//! for events a read already returned, nor while the item's last command is
//! still running. Its exit makes the runner look again.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use mergegate_core::agent::{self, AgentStore, DueWake};
use mergegate_core::item::ItemId;
use tokio::process::Command;
use tokio::sync::watch;
use tokio::time::{Duration, Instant};

use crate::HOME_ENV;
use crate::server::AppState;

/// How long a device's runner counts as there after its last poll ended: it
/// polls again at once, unless it is restarting.
const GRACE: Duration = Duration::from_secs(60);

pub(crate) struct Wakes {
    inner: Mutex<Inner>,
    /// Bumped by [`Wakes::recheck`]; every runner, the hub's and the
    /// devices' polls, looks again when it changes.
    changed: watch::Sender<u64>,
    /// The commands' PATH, set up by the first command this hub runs.
    path: OnceLock<Option<OsString>>,
}

impl Default for Wakes {
    fn default() -> Self {
        Self {
            inner: Mutex::default(),
            changed: watch::Sender::new(0),
            path: OnceLock::new(),
        }
    }
}

#[derive(Default)]
struct Inner {
    reading: HashMap<ItemId, usize>,
    /// The newest cursor a read returned to the item's agent.
    handed: HashMap<ItemId, u64>,
    /// Each item whose command is running, with the device running it; none
    /// for the hub.
    running: HashMap<ItemId, Option<String>>,
    /// The devices' runners, by device name.
    runners: HashMap<String, Runner>,
}

#[derive(Default)]
struct Runner {
    polls: usize,
    /// When the last poll ended, while none is open.
    ended: Option<Instant>,
}

/// Held for the length of a read of an item's feedback.
pub(crate) struct Reading<'a> {
    wakes: &'a Wakes,
    item: ItemId,
}

impl Drop for Reading<'_> {
    fn drop(&mut self) {
        let mut inner = self.wakes.lock();
        if let Some(count) = inner.reading.get_mut(&self.item) {
            *count -= 1;
            if *count == 0 {
                inner.reading.remove(&self.item);
            }
        }
        drop(inner);
        // Events logged after the read's last look still need a wake.
        self.wakes.recheck();
    }
}

/// Held for the length of a device runner's poll.
pub(crate) struct Polling<'a> {
    wakes: &'a Wakes,
    device: String,
}

impl Drop for Polling<'_> {
    fn drop(&mut self) {
        let mut inner = self.wakes.lock();
        if let Some(runner) = inner.runners.get_mut(&self.device) {
            runner.polls = runner.polls.saturating_sub(1);
            if runner.polls == 0 {
                runner.ended = Some(Instant::now());
            }
        }
    }
}

impl Wakes {
    pub(crate) fn reading(&self, item: &ItemId) -> Reading<'_> {
        *self.lock().reading.entry(item.clone()).or_default() += 1;
        Reading {
            wakes: self,
            item: item.clone(),
        }
    }

    /// Call before the [`Reading`] guard drops.
    pub(crate) fn handed(&self, item: &ItemId, cursor: u64) {
        let mut inner = self.lock();
        let handed = inner.handed.entry(item.clone()).or_default();
        *handed = (*handed).max(cursor);
    }

    /// Makes the runners look for due wakes, now or after their current look.
    pub(crate) fn recheck(&self) {
        self.changed.send_modify(|n| *n = n.wrapping_add(1));
    }

    pub(crate) fn changes(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }

    /// A poll from `device`'s runner began, which still runs the commands
    /// for `running`: the rest it was handed have exited. A hub that
    /// restarted since it handed them over learns them again here.
    pub(crate) fn polling(&self, device: &str, running: &HashSet<ItemId>) -> Polling<'_> {
        let mut inner = self.lock();
        inner
            .running
            .retain(|item, runner| runner.as_deref() != Some(device) || running.contains(item));
        for item in running {
            inner
                .running
                .entry(item.clone())
                .or_insert_with(|| Some(device.to_owned()));
        }
        let runner = inner.runners.entry(device.to_owned()).or_default();
        runner.polls += 1;
        runner.ended = None;
        drop(inner);
        // An item it let go of may have a wake waiting on it.
        self.recheck();
        Polling {
            wakes: self,
            device: device.to_owned(),
        }
    }

    /// Whether `device` has a runner polling, or one that polled moments ago.
    pub(crate) fn runs(&self, device: &str) -> bool {
        self.lock().runners.get(device).is_some_and(|runner| {
            runner.polls > 0 || runner.ended.is_some_and(|ended| ended.elapsed() < GRACE)
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic can't leave the maps disagreeing: each update touches one.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Looks on startup, after every append to the event log, and on every
/// [`Wakes::recheck`], until the hub stops.
pub(crate) async fn run(state: Arc<AppState>) {
    let mut log = state.events.log();
    let mut changes = state.wakes.changes();
    let mut stopping = state.events.stopping();
    loop {
        log.mark_unchanged();
        changes.mark_unchanged();
        let starts = {
            let state = Arc::clone(&state);
            state
                .clone()
                .blocking(move |store, _| take(store, &state.wakes, None))
                .await
        };
        match starts {
            Ok(starts) => {
                for wake in starts {
                    tokio::spawn(start(Arc::clone(&state), wake));
                }
            }
            Err(e) => tracing::error!("reading due wakes: {e}"),
        }
        tokio::select! {
            changed = log.changed() => if changed.is_err() { return },
            _ = changes.changed() => {}
            _ = stopping.wait_for(|stopping| *stopping) => return,
        }
    }
}

/// The wakes `device`'s runner is to start now, the hub's own for none, each
/// marked running and recorded as woken. Runs under the store's lock, which
/// is taken before the wakes' and never while holding them. A read registers
/// before it looks at the store and hands its events over before it
/// unregisters, so none slips between two checks.
pub(crate) fn take(
    store: &mut impl AgentStore,
    wakes: &Wakes,
    device: Option<&str>,
) -> Result<Vec<DueWake>, mergegate_core::store::StoreError> {
    let due = store.due_wakes()?;
    let mut inner = wakes.lock();
    let mut starts = Vec::new();
    for wake in due {
        // A command another machine runs doesn't hold the item back: its
        // agent moved here, and that machine's runner may never poll again
        // to let go of it.
        if wake.device.as_deref() != device
            || inner.reading.contains_key(&wake.item)
            || inner
                .running
                .get(&wake.item)
                .is_some_and(|runner| runner.as_deref() == device)
        {
            continue;
        }
        let read = inner
            .handed
            .get(&wake.item)
            .is_some_and(|&h| h >= wake.cursor);
        // Only the device's runner can see its worktree.
        let runnable = !read && (device.is_some() || worktree(&wake).is_some());
        store.set_woken(&wake.item, wake.cursor)?;
        if runnable {
            inner
                .running
                .insert(wake.item.clone(), device.map(str::to_owned));
            starts.push(wake);
        }
    }
    Ok(starts)
}

fn worktree(wake: &DueWake) -> Option<&Path> {
    let worktree = wake.worktree.as_deref().map(Path::new);
    match worktree {
        Some(path) if path.is_dir() => worktree,
        _ => {
            tracing::warn!(
                "not waking {}: its worktree {:?} is gone",
                wake.item,
                wake.worktree.as_deref().unwrap_or("(none recorded)"),
            );
            None
        }
    }
}

async fn start(state: Arc<AppState>, wake: DueWake) {
    run_command(&state, &wake).await;
    let mut inner = state.wakes.lock();
    if inner.running.get(&wake.item) == Some(&None) {
        inner.running.remove(&wake.item);
    }
    drop(inner);
    state.wakes.recheck();
}

async fn run_command(state: &AppState, wake: &DueWake) {
    let command = agent::wake_command(&wake.command, &wake.name);
    let Some(worktree) = wake.worktree.as_deref() else {
        return;
    };
    tracing::info!(
        event_code = "wake_started",
        "waking {} at cursor {}: {command}",
        wake.item,
        wake.cursor
    );
    // A file, not a pipe: the session it starts may outlive the hub and
    // write long after, and its stderr is unbounded.
    let log = state.wake_logs.join(format!("{}.log", wake.name));
    let stderr = match open_log(&log) {
        Ok(file) => file,
        Err(e) => {
            tracing::warn!("waking {}: opening {}: {e}", wake.item, log.display());
            return;
        }
    };
    let mut command_line = Command::new("sh");
    for name in agent::SESSION_MARKERS {
        command_line.env_remove(name);
    }
    if let Some(path) = state.wakes.path.get_or_init(|| path_env(&state.wake_bin)) {
        command_line.env("PATH", path);
    }
    let spawned = command_line
        .arg("-c")
        .arg(&command)
        .current_dir(worktree)
        .env("MG_ITEM", wake.name.to_string())
        .env("MG_CURSOR", wake.cursor.to_string())
        .env(HOME_ENV, &state.health.home)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        // Its own group, so a Ctrl-C to a foreground hub doesn't reach the
        // session it starts, which outlives the hub.
        .process_group(0)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            tracing::warn!("waking {}: {e}", wake.item);
            return;
        }
    };
    match child.wait().await {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!(
            "waking {} failed ({status}); its stderr is in {}",
            wake.item,
            log.display()
        ),
        Err(e) => tracing::warn!("waking {}: {e}", wake.item),
    }
}

/// The hub's PATH behind `bin`, which holds only a link to the `mg` running
/// this hub. `mg` in a command is then this hub's whatever PATH the hub
/// started with (a service manager's, or one where macOS's `/usr/bin/mg`
/// editor comes first), and every other command resolves as it would have.
pub(crate) fn path_env(bin: &Path) -> Option<OsString> {
    let linked = std::env::current_exe()
        .and_then(fs::canonicalize)
        .and_then(|exe| link_mg(bin, &exe));
    if let Err(e) = linked {
        tracing::warn!("linking {}/mg to this hub's mg: {e}", bin.display());
        return None;
    }
    own_first(bin, std::env::var_os("PATH").as_deref())
}

/// Points `bin/mg` at `exe`, over the link an older hub left, by renaming a
/// new link onto it so a running command never finds `bin/mg` missing.
fn link_mg(bin: &Path, exe: &Path) -> std::io::Result<()> {
    fs::create_dir_all(bin)?;
    let staged = bin.join(format!(".mg.{}", std::process::id()));
    let _ = fs::remove_file(&staged);
    std::os::unix::fs::symlink(exe, &staged)?;
    fs::rename(&staged, bin.join("mg"))
}

fn own_first(dir: &Path, path: Option<&OsStr>) -> Option<OsString> {
    let rest = path
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|entry| entry != dir);
    std::env::join_paths(std::iter::once(dir.to_owned()).chain(rest)).ok()
}

/// Truncated, so it holds the newest command's output only.
fn open_log(path: &Path) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    File::create(path)
}

#[cfg(test)]
#[path = "wake_tests.rs"]
mod tests;
