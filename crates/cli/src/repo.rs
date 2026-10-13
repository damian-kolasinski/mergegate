//! The worker's repo at the current directory, through the `git` binary with
//! the worker's own settings: their credentials and hooks apply to pushes.

use std::process::{Command, Output, Stdio};

use crate::output::CliError;

pub(crate) const REMOTE: &str = "mergegate";

fn output(args: &[&str]) -> Result<Output, CliError> {
    Command::new("git")
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
