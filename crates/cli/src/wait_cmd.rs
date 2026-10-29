//! `mg wait`: blocks until an item has events for its agent, except in a
//! Codex session. In Claude Code it waits without a timeout and leaves the
//! events for `mg feedback`. It outlasts a hub that restarts under it, and a
//! signal that stops it says how to resume.

mod notify;
mod session;
mod stop;

use std::time::{Duration, Instant};

use clap::ValueEnum;

use api::feedback::{Event, Feedback, Source};
use hub::Home;

use crate::client::Client;
use crate::feedback_cmd::{closed, read, report};
use crate::item_name;
use crate::output::{CliError, Outcome, exit};
use stop::Stopped;

/// Which events end the wait.
#[derive(Clone, Copy, ValueEnum)]
pub enum Wake {
    /// Any event
    Any,
    /// Only the operator's; the wait still returns every new event
    Operator,
}

impl Wake {
    fn by(self, events: &[Event]) -> bool {
        match self {
            Self::Any => !events.is_empty(),
            Self::Operator => events.iter().any(|e| e.source == Source::Operator),
        }
    }

    /// The words of the command that waits on as this one does.
    fn command(self) -> &'static [&'static str] {
        match self {
            Self::Any => &["wait"],
            Self::Operator => &["wait", "--for", "operator"],
        }
    }
}

/// How long a wait lasts unless told otherwise.
const TIMEOUT: u64 = 540;

/// The longest one request waits; a longer wait asks again.
const REQUEST: u64 = 3600;

/// How much longer than the wait it asked for the hub may take to answer.
/// Past it the hub is lost, as one is that went away across a tailnet
/// without closing the connection.
const ANSWER: Duration = Duration::from_secs(60);

/// How long a wait without a timeout keeps reconnecting to a hub it lost.
/// A restart, as on an upgrade, takes seconds; past this the hub is down.
const RECONNECT: Duration = Duration::from_secs(120);

/// The first pause before reconnecting, doubled after each failure up to
/// [`BACKOFF_MAX`].
const BACKOFF: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(15);

/// Set in the commands a Codex session runs. Codex checks on a running
/// command with a model turn, so a blocking wait there spends tokens until it
/// returns; there it reads once instead.
const CODEX_THREAD: &str = "CODEX_THREAD_ID";

/// Set to `1` in the commands a Claude Code session runs.
const CLAUDE_CODE: &str = "CLAUDECODE";

/// The agent host running the wait, which decides how it waits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Host {
    /// Reads once: Codex spends a model turn on every check of a running
    /// command.
    Codex,
    /// Waits without a timeout, and says events came instead of delivering
    /// them. A background command there costs nothing until it exits, but
    /// each exit starts a model turn that rereads the session, so a timeout
    /// only spends; and Claude Code can lose the command's output, which
    /// must not take the events with it.
    ClaudeCode,
    Other,
}

impl Host {
    fn detect() -> Self {
        if std::env::var_os(CODEX_THREAD).is_some_and(|id| !id.is_empty()) {
            Self::Codex
        } else if std::env::var_os(CLAUDE_CODE).is_some_and(|v| v == "1") {
            Self::ClaudeCode
        } else {
            Self::Other
        }
    }

    /// The timeout of a wait not given one; 0 is none.
    fn timeout(self) -> u64 {
        match self {
            Self::ClaudeCode => 0,
            Self::Codex | Self::Other => TIMEOUT,
        }
    }
}

/// `timeout` is the one given on the command line, if any.
pub fn run(
    item: String,
    wake: Wake,
    timeout: Option<u64>,
    since: Option<u64>,
    json: bool,
) -> Result<Outcome, CliError> {
    // First, before anything starts a thread.
    let watch = stop::watch(json, stopped(&item, wake, timeout, since));
    let waited = wait(&watch, item, wake, timeout, since);
    // An error prints as an outcome does, and a signal mustn't print over it.
    watch.done();
    waited
}

fn wait(
    watch: &stop::Watch,
    item: String,
    wake: Wake,
    given: Option<u64>,
    mut since: Option<u64>,
) -> Result<Outcome, CliError> {
    let at = |name: &str, since| stopped(name, wake, given, since);
    let host = Host::detect();
    let codex = host == Host::Codex;
    if host == Host::ClaudeCode {
        session::watch();
    }
    let timeout = given.unwrap_or(host.timeout());
    // A timeout too long to add is as good as none.
    let deadline = if codex {
        Some(Instant::now())
    } else {
        (timeout > 0)
            .then(|| Instant::now().checked_add(Duration::from_secs(timeout)))
            .flatten()
    };
    let mut retry = None;
    // In Claude Code a lost wait delivered nothing, so nothing needs recovery.
    if timeout == 0 && host == Host::Other {
        eprintln!(
            "mg wait: waiting on {item} without a timeout; if the host loses this command, \
             run mg feedback {item}, then mg resume {item} to recover any missed review"
        );
    }
    let home = crate::home().map_err(|e| resumable(e, &at(&item, since)))?;
    // A restart can be in progress before the first request, too. Keep its
    // retry budget through resolving the item and reading its feedback.
    let mut client = match connect(&home) {
        Ok(client) => client,
        Err(e) if lost(&e) && !codex => {
            let retry = retry.get_or_insert_with(|| Retry::new(deadline));
            reconnect(&home, e, retry, &at(&item, since))?
        }
        Err(e) => return Err(resumable(e, &at(&item, since))),
    };
    let found = loop {
        match item_name::find(&client, &item) {
            Ok(found) => break found,
            Err(e) if lost(&e) && !codex => {
                let retry = retry.get_or_insert_with(|| Retry::new(deadline));
                client = reconnect(&home, e, retry, &at(&item, since))?;
            }
            Err(e) => return Err(resumable(e, &at(&item, since))),
        }
    };
    let (item, name) = (found.id, found.name);
    let query = match wake {
        Wake::Any => "any",
        Wake::Operator => "operator",
    };
    loop {
        watch.waiting(at(&name, since));
        let wait = match deadline {
            None => REQUEST,
            Some(deadline) => deadline
                .saturating_duration_since(Instant::now())
                .as_secs_f64()
                .ceil() as u64,
        };
        let feedback = match read(&client, &item, since, Some((wait.min(REQUEST), query))) {
            Ok(feedback) => {
                retry = None;
                feedback
            }
            Err(e) if lost(&e) => {
                let retry = retry.get_or_insert_with(|| Retry::new(deadline));
                client = reconnect(&home, e, retry, &at(&name, since))?;
                continue;
            }
            Err(e) => return Err(resumable(e, &at(&name, since))),
        };
        // Closing an item logs an event, so the agent reads who closed it
        // and why with whatever came before. The hub also answers at its
        // deadline, with events that may not be the ones this waits for.
        if closed(feedback.state).is_some() || wake.by(&feedback.events) {
            // A closed item with nothing new delivers nothing either way.
            if host == Host::ClaudeCode && !feedback.events.is_empty() {
                return Ok(notify::arrived(&feedback, since));
            }
            return Ok(report(client, &feedback, since, wake.command()));
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let text = if codex {
                format!(
                    "no feedback on {} yet; mg wait doesn't wait in Codex: leave a wake with \
                     mg submit --wake and end your turn (the mergegate skill's waiting.md, Codex)",
                    feedback.name
                )
            } else {
                format!("no feedback on {} yet; call mg wait again", feedback.name)
            };
            return Ok(Outcome::done(&feedback, text).exit(exit::TIMEOUT));
        }
        since = next_since(since, &feedback);
    }
}

fn connect(home: &Home) -> Result<Client, CliError> {
    Ok(Client::connect(home)?.time_limit(Duration::from_secs(REQUEST) + ANSWER))
}

/// The hub went away or answered as one does while it restarts, rather
/// than refusing the wait.
fn lost(e: &CliError) -> bool {
    matches!(
        e.body.code.as_str(),
        "hub_unavailable"
            | "hub_unresponsive"
            | "unexpected_reply"
            | "tailscale_unavailable"
            | "store_failed"
    )
}

/// Reconnecting to a lost hub. It lasts until a read succeeds, not a
/// connect: a hub that answers its health but fails every read still runs
/// out of time, and its pauses still grow.
struct Retry {
    give_up: Instant,
    pause: Duration,
}

impl Retry {
    /// Until the wait's deadline, or for [`RECONNECT`] without one.
    fn new(deadline: Option<Instant>) -> Self {
        Self {
            give_up: deadline.unwrap_or_else(|| Instant::now() + RECONNECT),
            pause: BACKOFF,
        }
    }
}

/// Connects to the hub again after `e`, pausing longer after each failure.
/// Each failure is printed, so a wait that ends on one has said why.
fn reconnect(
    home: &Home,
    mut e: CliError,
    retry: &mut Retry,
    stopped: &Stopped,
) -> Result<Client, CliError> {
    loop {
        let left = retry.give_up.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(resumable(e, stopped));
        }
        let pause = retry.pause.min(left);
        eprintln!(
            "mg wait: {}; reconnecting in {}s",
            e.body.message,
            pause.as_secs_f64().ceil()
        );
        std::thread::sleep(pause);
        retry.pause = (retry.pause * 2).min(BACKOFF_MAX);
        match connect(home) {
            Ok(client) => return Ok(client),
            Err(next) if lost(&next) => e = next,
            Err(next) => return Err(resumable(next, stopped)),
        }
    }
}

/// `e`, with how to wait on from where this wait ended.
fn resumable(mut e: CliError, stopped: &Stopped) -> CliError {
    let resume = format!("once that's fixed, resume with: {}", stopped.resume);
    e.body.hint = Some(match e.body.hint.take() {
        Some(hint) => format!("{hint}; {resume}"),
        None => resume,
    });
    e
}

/// What a signal prints while the wait reads after `since`. Without it the
/// wait reads after the delivered cursor, which the hub keeps, so the same
/// command resumes it. A timeout given on the command line carries over, so
/// a wait run with `--timeout 0` resumes without one; without it, the resumed
/// wait takes its host's default again.
fn stopped(name: &str, wake: Wake, given: Option<u64>, since: Option<u64>) -> Stopped {
    let mut resume = format!("mg {} {name}", wake.command()[0]);
    for option in &wake.command()[1..] {
        resume.push_str(&format!(" {option}"));
    }
    if let Some(timeout) = given {
        resume.push_str(&format!(" --timeout {timeout}"));
    }
    if let Some(since) = since {
        resume.push_str(&format!(" --since {since}"));
    }
    Stopped {
        stopped: "",
        item: name.to_owned(),
        cursor: since,
        resume,
    }
}

/// Where the next request reads from. A `since` past the item's newest event
/// comes back as that event's cursor, and asking from the old one again
/// would skip an event logged between the two requests under it, as cursors
/// are hub-wide. Events that didn't end the wait are asked for again, since
/// the wait returns them once it ends.
fn next_since(since: Option<u64>, feedback: &Feedback) -> Option<u64> {
    since.map(|since| {
        if feedback.events.is_empty() {
            feedback.cursor
        } else {
            since
        }
    })
}

#[cfg(test)]
#[path = "wait_cmd_tests.rs"]
mod tests;
