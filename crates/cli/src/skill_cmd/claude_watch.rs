//! The Claude Stop hook waits outside the model and exits 2 only when the
//! hub has something actionable. Checkpoints suppress duplicate notifications,
//! not feedback: only an explicit feedback read acknowledges events.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use api::feedback::Attention;
use api::item::{Inbox, ItemState};
use serde::{Deserialize, Serialize};

use crate::client::{Client, segment};
use crate::output::{CliError, Outcome};
use crate::{events_cmd, repo};

const LIFETIME: Duration = Duration::from_secs(604_800);

#[derive(Deserialize)]
struct Input {
    session_id: String,
    cwd: PathBuf,
}

#[derive(Default, Deserialize, Serialize)]
struct Checkpoint {
    #[serde(default)]
    known: BTreeSet<String>,
    notified: BTreeMap<String, u64>,
    failed: bool,
}

pub(super) fn run() -> Result<Outcome, CliError> {
    run_hook().map_err(|error| error.exit(2))
}

fn run_hook() -> Result<Outcome, CliError> {
    let input: Input = serde_json::from_reader(std::io::stdin().take(65_536)).map_err(failed)?;
    if input.session_id.is_empty()
        || input.session_id.len() > 128
        || !input
            .session_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(failed("invalid Claude session ID"));
    }
    std::env::set_current_dir(&input.cwd).map_err(failed)?;
    let Some(remote) = repo::read(&["remote", "get-url", repo::REMOTE])? else {
        return Ok(silent());
    };
    let Some(project) = super::session_start::project(&remote) else {
        return Ok(silent());
    };
    let Some(root) = repo::read(&["rev-parse", "--show-toplevel"])? else {
        return Ok(silent());
    };
    let root = Path::new(&root).canonicalize().map_err(failed)?;
    let home = crate::home()?;
    let directory = home.root().join("claude-hooks");
    fs::create_dir_all(&directory).map_err(failed)?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join(format!("{}.lock", input.session_id)))
        .map_err(failed)?;
    match lock.try_lock() {
        Ok(()) => (),
        Err(std::fs::TryLockError::WouldBlock) => return Ok(silent()),
        Err(error) => return Err(failed(error)),
    }
    let path = directory.join(format!("{}.json", input.session_id));
    let mut checkpoint: Checkpoint = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(failed)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Checkpoint::default(),
        Err(error) => return Err(failed(error)),
    };
    crate::wait_cmd::session::watch();
    let deadline = Instant::now() + LIFETIME;
    let mut cursor = None;
    let mut known = checkpoint.known.clone();
    loop {
        if Instant::now() >= deadline {
            return Err(failed(
                "review watcher reached its seven-day limit; end this turn to rearm it",
            ));
        }
        match poll(
            &home,
            project,
            &root,
            &mut cursor,
            &mut known,
            &checkpoint.notified,
        ) {
            Ok((notifications, open)) => {
                checkpoint.known.clone_from(&known);
                checkpoint.failed = false;
                if !notifications.is_empty() {
                    let mut text = Vec::new();
                    for (id, attention) in notifications {
                        checkpoint
                            .notified
                            .insert(id, attention.cursor.expect("actionable"));
                        text.push(format!(
                            "Run `mg feedback {}` and act on the feedback.",
                            attention.name
                        ));
                    }
                    save(&path, &checkpoint)?;
                    return Ok(Outcome::done(&text, text.join("\n")).exit(2));
                }
                save(&path, &checkpoint)?;
                if !open {
                    return Ok(silent());
                }
            }
            Err(error) => {
                checkpoint.known.clone_from(&known);
                if !checkpoint.failed {
                    checkpoint.failed = true;
                    save(&path, &checkpoint)?;
                    return Err(error.hint(
                        "Review watcher failed; run mg doctor. End the turn to retry watching.",
                    ));
                }
                // The first failure already woke the session. Keep retrying
                // after its next Stop without a new model turn per outage.
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    }
}

fn poll(
    home: &hub::Home,
    project: &str,
    root: &Path,
    cursor: &mut Option<u64>,
    known: &mut BTreeSet<String>,
    notified: &BTreeMap<String, u64>,
) -> Result<(Vec<(String, Attention)>, bool), CliError> {
    let client = Client::connect(home)?.time_limit(Duration::from_secs(40));
    // Start the log before discovery so an event arriving during discovery
    // cannot fall between the snapshot and the next long poll.
    let log = events_cmd::read(&client, *cursor, if cursor.is_some() { 30 } else { 0 })?;
    *cursor = Some(log.cursor);
    let inbox: Inbox = client.get("/api/inbox")?;
    for item in inbox
        .needs_you
        .into_iter()
        .chain(inbox.with_agents)
        .chain(inbox.merging)
        .chain(inbox.closed)
    {
        if item.name.split_once('#').map(|(name, _)| name) == Some(project) {
            known.insert(item.id);
        }
    }
    let mut notifications = Vec::new();
    let mut open = false;
    let mut remove = Vec::new();
    for id in known.iter() {
        let prefix = format!("/api/items/{}", segment(id));
        let attention: Attention = client.get_query(
            &format!("{prefix}/attention"),
            &[("worktree", root.to_string_lossy().as_ref())],
        )?;
        if !attention.watching {
            remove.push(id.clone());
            continue;
        }
        open |= !matches!(attention.state, ItemState::Merged | ItemState::Abandoned);
        if attention
            .cursor
            .is_some_and(|cursor| cursor > *notified.get(id).unwrap_or(&0))
        {
            notifications.push((id.clone(), attention));
        }
    }
    for id in remove {
        known.remove(&id);
    }
    Ok((notifications, open))
}

fn save(path: &Path, checkpoint: &Checkpoint) -> Result<(), CliError> {
    let mut file = tempfile::NamedTempFile::new_in(path.parent().expect("checkpoint directory"))
        .map_err(failed)?;
    serde_json::to_writer(file.as_file_mut(), checkpoint).map_err(failed)?;
    file.flush().map_err(failed)?;
    file.persist(path).map_err(failed)?;
    Ok(())
}

fn silent() -> Outcome {
    Outcome::done(&(), "")
}

fn failed(error: impl std::fmt::Display) -> CliError {
    CliError::new("claude_watch_failed", error.to_string())
}
