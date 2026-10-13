use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use mergegate_core::git::GitError;

/// The `git` binary, with only the repos' own config applying: no `GIT_*`
/// variables from the hub's environment, no system or global config, no
/// hooks and no prompts.
pub(crate) fn git(home: &Path) -> Command {
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
        .args(["-c", "core.hooksPath=/dev/null"]);
    cmd
}

/// Runs `git` with `args` and returns its stdout. `doing` names the step in
/// errors, as in "fetching main from /src/app failed: ...".
pub(crate) fn run<I, S>(home: &Path, doing: &str, args: I) -> Result<String, GitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = output(home, doing, args)?;
    if !output.status.success() {
        return Err(failed(doing, &output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Like [`run`], for commands that answer "none" by exiting with 1 and
/// printing nothing, as `git merge-base` does.
pub(crate) fn run_optional<I, S>(
    home: &Path,
    doing: &str,
    args: I,
) -> Result<Option<String>, GitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = output(home, doing, args)?;
    if output.status.code() == Some(1) && output.stderr.trim_ascii().is_empty() {
        return Ok(None);
    }
    if !output.status.success() {
        return Err(failed(doing, &output));
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
}

fn output<I, S>(home: &Path, doing: &str, args: I) -> Result<Output, GitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    git(home)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| GitError::Failed(format!("{doing} failed: can't run git: {e}")))
}

fn failed(doing: &str, output: &Output) -> GitError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let why = match stderr.trim() {
        "" => output.status.to_string(),
        stderr => stderr.to_owned(),
    };
    GitError::Failed(format!("{doing} failed: {why}"))
}
