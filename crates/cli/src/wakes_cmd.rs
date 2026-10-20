//! `mg wakes run`: on a machine `mg connect` points at another hub, runs the
//! wake commands its agents left with `mg submit --wake`, which the hub hands
//! over as their items get events from the operator. The hub's machine needs
//! none: the hub runs those left there itself. `mg connect` installs it as a
//! service.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use api::wake::{Due, Wake};
use clap::Subcommand;
use hub::Home;
use mergegate_core::agent::SESSION_MARKERS;
use serde::Serialize;

use crate::client::Client;
use crate::output::{CliError, Outcome};

/// Seconds one poll waits for a wake. The runner sees an upgraded `mg`, or
/// a `mg connect` to another hub, between two.
const POLL: &str = "300";

/// Seconds one poll waits while commands run: the next one tells the hub
/// which have exited, so the item can be woken again.
const POLL_RUNNING: &str = "15";

/// Longer than any poll takes, so only a hub gone without a word ends one.
const POLL_LIMIT: Duration = Duration::from_secs(330);

/// How long to wait before trying an unreachable or refusing hub again.
const RETRY: Duration = Duration::from_secs(30);

#[derive(Subcommand)]
pub enum WakesCommand {
    /// Run the wake commands the hub hands this machine, until stopped or
    /// mg connect --local
    Run,
}

#[derive(Serialize)]
struct Stopped {
    connected: bool,
}

pub fn run(command: WakesCommand) -> Result<Outcome, CliError> {
    match command {
        WakesCommand::Run => run_wakes(&crate::home()?),
    }
}

fn run_wakes(home: &Home) -> Result<Outcome, CliError> {
    let exe = Exe::this();
    let path = hub::wake_path(home);
    let running = Running::default();
    loop {
        // Read anew each time, so `mg connect` to another hub needs no
        // restart, and `mg connect --local` ends it.
        if home.read_connection().is_none() {
            let text = "mg uses the hub on this machine, which runs its wakes itself";
            return Ok(Outcome::done(&Stopped { connected: false }, text));
        }
        let ids = running.ids();
        // Not while commands run, whose exits only this process sees.
        if let Some(exe) = exe.as_ref().filter(|exe| ids.is_empty() && exe.replaced()) {
            exe.restart();
        }
        let wait = if ids.is_empty() { POLL } else { POLL_RUNNING };
        let due = Client::connect(home).and_then(|client| {
            let client = client.time_limit(POLL_LIMIT);
            client.get_query::<Due>("/api/wakes", &[("wait", wait), ("running", &ids)])
        });
        match due {
            Ok(due) => {
                for wake in due.wakes {
                    start(home, path.as_deref(), wake, &running);
                }
            }
            Err(e) => {
                eprintln!(
                    "mg wakes: {}; trying again in {}s",
                    e.body.message,
                    RETRY.as_secs()
                );
                thread::sleep(RETRY);
            }
        }
    }
}

/// The items whose commands run, by ID, which each poll tells the hub so it
/// hands over no other command for them meanwhile.
#[derive(Default, Clone)]
struct Running(Arc<Mutex<HashSet<String>>>);

impl Running {
    fn ids(&self) -> String {
        let ids = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        ids.iter().cloned().collect::<Vec<_>>().join(",")
    }

    fn insert(&self, id: &str) {
        let mut ids = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        ids.insert(id.to_owned());
    }

    fn remove(&self, id: &str) {
        let mut ids = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        ids.remove(id);
    }
}

/// Runs `wake`'s command as the hub runs its own machine's: `sh -c` in the
/// worktree with `path` as its PATH, its stderr in the item's log, in a
/// process group of its own so the session it starts outlives this runner.
fn start(home: &Home, path: Option<&OsStr>, wake: Wake, running: &Running) {
    let worktree = Path::new(&wake.worktree);
    if !worktree.is_dir() {
        eprintln!(
            "mg wakes: not waking {}: its worktree {} is gone",
            wake.item, wake.worktree
        );
        return;
    }
    let log = home.wake_logs().join(format!("{}.log", wake.item));
    let stderr = match open_log(&log) {
        Ok(file) => file,
        Err(e) => {
            eprintln!(
                "mg wakes: waking {}: opening {}: {e}",
                wake.item,
                log.display()
            );
            return;
        }
    };
    eprintln!(
        "mg wakes: waking {} at cursor {}: {}",
        wake.item, wake.cursor, wake.command
    );
    let mut command = Command::new("sh");
    for name in SESSION_MARKERS {
        command.env_remove(name);
    }
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let spawned = command
        .arg("-c")
        .arg(&wake.command)
        .current_dir(worktree)
        .env(hub::HOME_ENV, home.root())
        .env("MG_ITEM", &wake.item)
        .env("MG_CURSOR", wake.cursor.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .process_group(0)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            eprintln!("mg wakes: waking {}: {e}", wake.item);
            return;
        }
    };
    running.insert(&wake.id);
    let running = running.clone();
    thread::spawn(move || {
        match child.wait() {
            Ok(status) if status.success() => {}
            Ok(status) => eprintln!(
                "mg wakes: waking {} failed ({status}); its stderr is in {}",
                wake.item,
                log.display()
            ),
            Err(e) => eprintln!("mg wakes: waking {}: {e}", wake.item),
        }
        running.remove(&wake.id);
    });
}

/// Truncated, so it holds the newest command's output only.
fn open_log(path: &Path) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    File::create(path)
}

/// The `mg` binary this runs, by the path an upgrade moves, as it was when
/// it started.
struct Exe {
    path: PathBuf,
    identity: (u64, u64, i64),
}

impl Exe {
    fn this() -> Option<Self> {
        let path = crate::hub_cmd::service::this_mg().ok()?;
        let identity = identity(&path)?;
        Some(Self { path, identity })
    }

    /// Whether an upgrade put another binary at its path.
    fn replaced(&self) -> bool {
        identity(&self.path).is_some_and(|now| now != self.identity)
    }

    /// Runs the new binary in this process, so a service needs no restart
    /// for the runner to speak the upgraded hub's API.
    fn restart(&self) {
        eprintln!(
            "mg wakes: {} was upgraded; restarting on it",
            self.path.display()
        );
        let e = Command::new(&self.path)
            .args(std::env::args_os().skip(1))
            .exec();
        eprintln!("mg wakes: restarting on {}: {e}", self.path.display());
    }
}

fn identity(path: &Path) -> Option<(u64, u64, i64)> {
    let meta = fs::metadata(path).ok()?;
    Some((meta.dev(), meta.ino(), meta.mtime()))
}
