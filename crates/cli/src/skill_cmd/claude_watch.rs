//! The Claude Stop hook waits outside the model and exits 2 only when the
//! hub has something actionable. Checkpoints suppress duplicate notifications,
//! not feedback: only an explicit feedback read acknowledges events.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use api::attention::{Watch, Watched, Watching};
use api::item::ItemState;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::output::{CliError, Outcome};
use crate::repo;

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
    let local = local.as_deref();
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
                    for watched in notifications {
                        text.push(format!(
                            "Run `mg feedback {}` and act on the feedback.",
                            watched.name
                        ));
                        checkpoint
                            .notified
                            .insert(watched.id, watched.cursor.expect("actionable"));
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

/// The worktree of the session's directory, when it is a mergegate clone.
fn local() -> Result<Option<PathBuf>, CliError> {
    let Some(remote) = repo::read(&["remote", "get-url", repo::REMOTE])? else {
        return Ok(None);
    };
    if super::session_start::project(&remote).is_none() {
        return Ok(None);
    }
    let Some(root) = repo::read(&["rev-parse", "--show-toplevel"])? else {
        return Ok(None);
    };
    Path::new(&root).canonicalize().map(Some).map_err(failed)
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

/// One wait on the hub for every item the session watches: those its clone
/// owns, the ones it found there before, and its own submits. Returns the
/// items with something new worth a turn, and whether any is still open.
fn poll(
    home: &hub::Home,
    local: Option<&Path>,
    items: &Path,
    cursor: &mut Option<u64>,
    known: &mut BTreeSet<String>,
    notified: &BTreeMap<String, u64>,
) -> Result<(Vec<Watched>, bool), CliError> {
    let client = Client::connect(home)?.time_limit(Duration::from_secs(40));
    let root = local.map(|root| root.to_string_lossy().into_owned());
    // An item found in the clone stays watched there once it's no longer
    // open or closed today; the session's own submits name their worktree,
    // wherever it ends turns.
    let mut named = BTreeMap::new();
    if let Some(root) = &root {
        named.extend(known.iter().map(|id| (id.clone(), root.clone())));
    }
    named.extend(submitted(items)?);
    let watch = Watch {
        worktree: root.clone(),
        items: named
            .into_iter()
            .map(|(id, worktree)| api::attention::Submitted { id, worktree })
            .collect(),
    };
    // The first read starts from now, at once; later ones wait after it, so
    // nothing logged between two reads goes unseen.
    let since = cursor.map(|cursor| cursor.to_string());
    let query = match &since {
        Some(since) => vec![("since", since.as_str()), ("wait", "30")],
        None => Vec::new(),
    };
    let watching: Watching = client.post_query("/api/attention", &query, &watch)?;
    *cursor = Some(watching.cursor);
    if root.is_some() {
        *known = watching.items.iter().map(|w| w.id.clone()).collect();
    }
    let open = watching
        .items
        .iter()
        .any(|w| !matches!(w.state, ItemState::Merged | ItemState::Abandoned));
    let notifications = watching
        .items
        .into_iter()
        .filter(|w| {
            w.cursor
                .is_some_and(|cursor| cursor > *notified.get(&w.id).unwrap_or(&0))
        })
        .collect();
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
