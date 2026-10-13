use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

use mergegate_core::git::GitError;

/// Runs the `git` binary and returns its stdout. Only the repos' own config
/// applies: no `GIT_*` variables from the hub's environment, no system or
/// global config, no hooks and no prompts. `doing` names the step in errors,
/// as in "fetching main from /src/app failed: ...".
pub(crate) fn run<I, S>(home: &Path, doing: &str, args: I) -> Result<String, GitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.as_encoded_bytes().starts_with(b"GIT_") {
            cmd.env_remove(key);
        }
    }
    cmd.env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home)
        .env("LC_ALL", "C")
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .stdin(Stdio::null());
    let output = cmd
        .output()
        .map_err(|e| GitError::Failed(format!("{doing} failed: can't run git: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitError::Failed(format!(
            "{doing} failed: {}",
            stderr.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
