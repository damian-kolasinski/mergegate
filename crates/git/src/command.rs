use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use mergegate_core::git::GitError;

/// The `git` binary, with only the repos' own config applying: no `GIT_*`
/// variables from the hub's environment, no system or global config, no
/// hooks and no prompts.
pub(crate) fn git(home: &Path) -> Command {
    let mut cmd = Command::new(crate::binary());
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

/// The `git` binary as the operator runs it, for acting on their repo on
/// their behalf: their config, identity and hooks apply. Only the variables
/// that point git at a repo are dropped, since the hub may have been started
/// inside one.
pub(crate) fn operator() -> Command {
    let mut cmd = Command::new(crate::binary());
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_NAMESPACE",
    ] {
        cmd.env_remove(key);
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C");
    cmd
}

/// `doing` names the step in errors, as in "fetching main from /src/app
/// failed: ...".
pub(crate) fn run<I, S>(home: &Path, doing: &str, args: I) -> Result<String, GitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = git(home);
    cmd.args(args);
    run_command(cmd, doing)
}

/// Like [`run`], for a command [`git`] or [`operator`] made and the caller
/// set up further.
pub(crate) fn run_command(cmd: Command, doing: &str) -> Result<String, GitError> {
    let output = output_of(cmd, doing)?;
    if !output.status.success() {
        return Err(failed(doing, &output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Like [`run`], with `input` on stdin, for lists too long for arguments.
pub(crate) fn run_with_input<I, S>(
    home: &Path,
    doing: &str,
    args: I,
    input: &[u8],
) -> Result<String, GitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = git(home);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let cant_run =
        |e: std::io::Error| GitError::Failed(format!("{doing} failed: can't run git: {e}"));
    let mut child = cmd.spawn().map_err(cant_run)?;
    let mut stdin = child.stdin.take().expect("piped");
    // Written from its own thread: git answers as it reads, and would block
    // on a full stdout pipe while we block on a full stdin one.
    let input = input.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let output = child.wait_with_output().map_err(cant_run)?;
    let written = writer.join().expect("stdin writer panicked");
    if !output.status.success() {
        return Err(failed(doing, &output));
    }
    written.map_err(cant_run)?;
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
    let mut cmd = git(home);
    cmd.args(args);
    output_of(cmd, doing)
}

/// Runs `cmd` whatever its exit status, for commands whose failures say
/// something, as `git merge-tree` exiting with 1 on a conflict does.
pub(crate) fn output_of(mut cmd: Command, doing: &str) -> Result<Output, GitError> {
    cmd.stdin(Stdio::null())
        .output()
        .map_err(|e| GitError::Failed(format!("{doing} failed: can't run git: {e}")))
}

pub(crate) fn failed(doing: &str, output: &Output) -> GitError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let why = match stderr.trim() {
        "" => output.status.to_string(),
        stderr => stderr.to_owned(),
    };
    GitError::Failed(format!("{doing} failed: {why}"))
}
