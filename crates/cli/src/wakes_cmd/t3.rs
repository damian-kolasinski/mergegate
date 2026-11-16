use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::output::{CliError, Outcome};

#[derive(Serialize)]
struct Woken {
    thread: String,
}

pub(crate) fn detected() -> bool {
    std::env::var("T3CODE_THREAD_ID").is_ok_and(|thread| !thread.is_empty())
}

// T3's shared Codex process runs session hooks before the per-thread shell
// environment exists. Its CLI path identifies the host for guidance only;
// registration still requires the actual thread ID from a command shell.
pub(crate) fn host_detected() -> bool {
    detected() || std::env::var("T3CODE_CLI_PATH").is_ok_and(|cli| !cli.is_empty())
}

pub(crate) fn registration(wake: Option<String>) -> Option<String> {
    if wake.is_some() {
        return wake;
    }
    let thread = std::env::var("T3CODE_THREAD_ID")
        .ok()
        .filter(|thread| !thread.is_empty())?;
    let cli = std::env::var("T3CODE_CLI_PATH")
        .ok()
        .filter(|cli| !cli.is_empty())
        .unwrap_or_else(|| "t3".into());
    let mut command = format!("mg wakes t3 {} --cli {}", quote(&thread), quote(&cli));
    if let Ok(home) = std::env::var("T3CODE_HOME")
        && !home.is_empty()
    {
        command.push_str(&format!(" --base-dir {}", quote(&home)));
    }
    Some(command)
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn run(
    thread: String,
    cli: PathBuf,
    base_dir: Option<PathBuf>,
) -> Result<Outcome, CliError> {
    let item = std::env::var("MG_ITEM").unwrap_or_else(|_| "the item".into());
    let cursor = std::env::var("MG_CURSOR")
        .map(|cursor| format!(" (newest cursor {cursor})"))
        .unwrap_or_default();
    let message = format!(
        "mergegate: {item} has events you haven't read{cursor}. Run `mg feedback {item}` \
         and act on what it returns; the wake stays set, so end your turn when you're done \
         instead of waiting."
    );
    let mut command = Command::new(&cli);
    command.args(["thread", "send", &thread, &message]);
    // The runner may belong to a different T3 instance than the submitter.
    command.env_remove("T3CODE_HOME");
    if let Some(base_dir) = base_dir {
        command.arg("--base-dir").arg(base_dir);
    }
    let status = command.stdout(Stdio::null()).status().map_err(|error| {
        CliError::new(
            "wake_failed",
            format!("can't run {}: {error}", cli.display()),
        )
        .hint("pass --cli with the path from T3CODE_CLI_PATH")
    })?;
    if !status.success() {
        return Err(CliError::new(
            "wake_failed",
            format!("waking T3 Code thread {thread} failed ({status}); its stderr says why"),
        ));
    }
    let text = format!("queued a message for T3 Code thread {thread}");
    Ok(Outcome::done(&Woken { thread }, text))
}
