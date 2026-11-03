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

/// One line of a session's `.items` file: an item `mg submit` sent from the
/// session, and the worktree it was sent from, which the hub matches.
#[derive(Deserialize, Serialize)]
struct Submitted {
    id: String,
    worktree: String,
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

/// Notes an item `mg submit` sent from a Claude Code session, so that the
/// session's hook watches it from wherever the session ends its turns: the
/// shell's directory then, which the hook is given, needn't be `worktree`.
/// Outside a session it hooks, it does nothing.
pub(crate) fn remember(home: &hub::Home, id: &str, worktree: &str) -> Result<(), CliError> {
    let Some(session) = std::env::var("CLAUDE_CODE_SESSION_ID")
        .ok()
        .filter(|session| valid(session))
    else {
        return Ok(());
    };
    if print_mode() {
        return Ok(());
    }
    let directory = home.root().join("claude-hooks");
    fs::create_dir_all(&directory).map_err(failed)?;
    let mut line = serde_json::to_string(&Submitted {
        id: id.to_owned(),
        worktree: worktree.to_owned(),
    })
    .map_err(failed)?;
    line.push('\n');
    // One short append per submit, so concurrent submits don't interleave
    // and the hook, which only reads, needs no lock with them.
    File::options()
        .create(true)
        .append(true)
        .open(directory.join(format!("{session}.items")))
        .and_then(|mut file| file.write_all(line.as_bytes()))
        .map_err(failed)
}

fn run_hook() -> Result<Outcome, CliError> {
    let input: Input = serde_json::from_reader(std::io::stdin().take(65_536)).map_err(failed)?;
    if !valid(&input.session_id) {
        return Err(failed("invalid Claude session ID"));
    }
    if print_mode() {
        return Ok(silent());
    }
    std::env::set_current_dir(&input.cwd).map_err(failed)?;
    let local = local()?;
    let home = crate::home()?;
    let directory = home.root().join("claude-hooks");
    let items = directory.join(format!("{}.items", input.session_id));
    if local.is_none() && !items.exists() {
        return Ok(silent());
    }
    let local = local
        .as_ref()
        .map(|(project, root)| (project.as_str(), root.as_path()));
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
    let grace = reconnect();
    let mut lost_since = None;
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
            local,
            &items,
            &mut cursor,
            &mut known,
            &checkpoint.notified,
        ) {
            Ok((notifications, open)) => {
                lost_since = None;
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
                // A hub restart, as on every upgrade, takes seconds and would
                // otherwise cost each watching session a turn; `mg wait`
                // rides one out the same way.
                let lost_for = crate::wait_cmd::lost(&error)
                    .then(|| lost_since.get_or_insert_with(Instant::now).elapsed());
                if !checkpoint.failed && lost_for.is_none_or(|lost| lost >= grace) {
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

/// The project and worktree of the session's directory, when it is a
/// mergegate clone.
fn local() -> Result<Option<(String, PathBuf)>, CliError> {
    let Some(remote) = repo::read(&["remote", "get-url", repo::REMOTE])? else {
        return Ok(None);
    };
    let Some(project) = super::session_start::project(&remote) else {
        return Ok(None);
    };
    let Some(root) = repo::read(&["rev-parse", "--show-toplevel"])? else {
        return Ok(None);
    };
    let root = Path::new(&root).canonicalize().map_err(failed)?;
    Ok(Some((project.to_owned(), root)))
}

/// Every item the session submitted, by ID, with the worktree of its latest
/// submit. A line cut short by a crash is skipped.
fn submitted(items: &Path) -> Result<BTreeMap<String, String>, CliError> {
    match fs::read_to_string(items) {
        Ok(text) => Ok(text
            .lines()
            .filter_map(|line| serde_json::from_str::<Submitted>(line).ok())
            .map(|entry| (entry.id, entry.worktree))
            .collect()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(failed(error)),
    }
}

fn poll(
    home: &hub::Home,
    local: Option<(&str, &Path)>,
    items: &Path,
    cursor: &mut Option<u64>,
    known: &mut BTreeSet<String>,
    notified: &BTreeMap<String, u64>,
) -> Result<(Vec<(String, Attention)>, bool), CliError> {
    let client = Client::connect(home)?.time_limit(Duration::from_secs(40));
    // Start the log before discovery so an event arriving during discovery
    // cannot fall between the snapshot and the next long poll.
    let log = events_cmd::read(&client, *cursor, if cursor.is_some() { 30 } else { 0 })?;
    *cursor = Some(log.cursor);
    let mut watched = BTreeMap::new();
    if let Some((project, root)) = local {
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
        let root = root.to_string_lossy();
        watched.extend(known.iter().map(|id| (id.clone(), root.to_string())));
    }
    // The session's own submits name their worktree, wherever it ends turns.
    watched.extend(submitted(items)?);
    let mut notifications = Vec::new();
    let mut open = false;
    let mut remove = Vec::new();
    for (id, worktree) in &watched {
        let prefix = format!("/api/items/{}", segment(id));
        let attention: Attention =
            client.get_query(&format!("{prefix}/attention"), &[("worktree", worktree)])?;
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

/// How long a lost hub may stay away before the session hears of it. Tests
/// shorten it with `MG_WATCH_RECONNECT_MS`.
fn reconnect() -> Duration {
    std::env::var("MG_WATCH_RECONNECT_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map_or(crate::wait_cmd::RECONNECT, Duration::from_millis)
}

fn valid(session: &str) -> bool {
    !session.is_empty()
        && session.len() <= 128
        && session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Claude Code backgrounds an asyncRewake hook only in an interactive
/// session or one with streaming input; `claude -p` runs it in line, so
/// watching there would hold the run open until the review comes. Print
/// mode is the entrypoint `sdk-cli`; the SDKs name themselves.
fn print_mode() -> bool {
    std::env::var_os("CLAUDE_CODE_ENTRYPOINT").is_some_and(|entry| entry == "sdk-cli")
}

fn silent() -> Outcome {
    Outcome::done(&(), "")
}

fn failed(error: impl std::fmt::Display) -> CliError {
    CliError::new("claude_watch_failed", error.to_string())
}
