//! The worker's repo at the current directory, through the `git` binary with
//! the worker's own settings: their credentials and hooks apply to pushes.

use std::process::{Command, Output, Stdio};

use crate::output::CliError;

pub(crate) const REMOTE: &str = "mergegate";

fn output(args: &[&str]) -> Result<Output, CliError> {
    Command::new(git::binary())
        .args(args)
        // Never prompt: agents can't answer.
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| CliError::new("git_unavailable", format!("can't run git: {e}")))
}

/// stdout, trimmed, or `None` when git fails.
pub(crate) fn read(args: &[&str]) -> Result<Option<String>, CliError> {
    let output = output(args)?;
    Ok(output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned()))
}

/// `doing` names the step if it fails.
pub(crate) fn run(doing: &str, args: &[&str]) -> Result<(), CliError> {
    let output = output(args)?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(CliError::new(
        "git_failed",
        format!("{doing} failed: {}", stderr.trim()),
    ))
}

/// Has the clone's git give up on a transfer with the hub at `url` that
/// stalls, as one does when the hub's machine leaves the tailnet or its
/// firewall drops the hub, instead of waiting on it for good: `mg submit`'s
/// push, and the `git fetch mergegate` before it. Two minutes, since a fetch
/// is answered only once the hub has landed any item it is landing and
/// fetched the target from upstream. Limits the operator set for the hub, or
/// for every host, stay. Best effort: a clone whose config can't be written
/// pushes all the same, unbounded.
pub(crate) fn bound_stalls(url: &str) {
    for (name, value) in [("lowSpeedLimit", "1"), ("lowSpeedTime", "120")] {
        if let Ok(None) = read(&["config", "--get-urlmatch", &format!("http.{name}"), url]) {
            let _ = output(&["config", "--local", &format!("http.{url}.{name}"), value]);
        }
    }
}

/// Fails unless the current directory is in a git repo.
pub(crate) fn require() -> Result<(), CliError> {
    match read(&["rev-parse", "--git-dir"])? {
        Some(_) => Ok(()),
        None => Err(CliError::new(
            "not_a_repo",
            "the current directory is not inside a git repository",
        )
        .hint("run mg in the clone you work in")),
    }
}
