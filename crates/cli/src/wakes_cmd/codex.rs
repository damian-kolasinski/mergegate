//! `mg wakes codex`: the wake command a Codex agent leaves with `mg submit
//! --wake`, which queues a message for its thread and starts a turn of it.
//! The script ships inside `mg`, so a wake needs no path to a skill folder,
//! which the stub agents load doesn't have.

use std::process::Command;

use serde::Serialize;

use crate::output::{CliError, Outcome};

const SCRIPT: &str = include_str!("codex_wake.py");

#[derive(Serialize)]
struct Woken {
    thread: String,
}

pub(super) fn run(thread: String) -> Result<Outcome, CliError> {
    // Isolated: the hub runs a wake in the agent's worktree, whose files
    // must not shadow the modules the script imports.
    let status = Command::new("python3")
        .args(["-I", "-c", SCRIPT, &thread])
        .status()
        .map_err(|e| {
            CliError::new("wake_failed", format!("can't run python3: {e}"))
                .hint("install Python 3, which waking a Codex thread runs")
        })?;
    if !status.success() {
        return Err(CliError::new(
            "wake_failed",
            format!("waking Codex thread {thread} failed ({status}); its stderr says why"),
        ));
    }
    let text = format!("queued a message for Codex thread {thread}");
    Ok(Outcome::done(&Woken { thread }, text))
}
