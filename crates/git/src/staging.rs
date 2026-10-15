//! Each project's staging repo: a bare repo the hub owns, where items are
//! merged and checked before anything touches the operator's repo.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use mergegate_core::git::{GitError, Upstream};

use crate::command::{self, run, run_command};
use crate::remote::{authorize, describe, location};

pub(crate) fn sync(
    home: &Path,
    dir: &Path,
    upstream: &Upstream,
    target: &str,
) -> Result<(), GitError> {
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| GitError::Failed(format!("creating {} failed: {e}", parent.display())))?;
    }
    let existed = dir.exists();
    let synced = mirror(home, dir, upstream, target);
    if synced.is_err() && !existed {
        // Don't leave a half-made repo that looks like a project's staging.
        let _ = std::fs::remove_dir_all(dir);
    }
    synced
}

fn mirror(home: &Path, dir: &Path, upstream: &Upstream, target: &str) -> Result<(), GitError> {
    let arg = OsStr::new;
    // Re-running init on an existing repo is safe, and finishes one an
    // earlier run left half made.
    run(
        home,
        "creating the staging repo",
        [arg("init"), arg("--bare"), arg("--quiet"), dir.as_os_str()],
    )?;
    let mut git_dir = OsString::from("--git-dir=");
    git_dir.push(dir);
    let branch = format!("refs/heads/{target}");
    // HEAD names the target of the last sync. When the target changed, drop
    // the old one: it would go stale, and `release/1.0` blocks a new
    // `release`. Only that ref: other branches in staging aren't ours to drop.
    let previous = run(
        home,
        "reading the staging repo's target",
        [
            git_dir.as_os_str(),
            arg("symbolic-ref"),
            arg("--quiet"),
            arg("HEAD"),
        ],
    )
    .ok()
    .map(|head| head.trim().to_owned())
    .filter(|head| !head.is_empty() && *head != branch);
    if let Some(previous) = previous {
        run(
            home,
            "removing the previous target from the staging repo",
            [
                git_dir.as_os_str(),
                arg("update-ref"),
                arg("-d"),
                arg(&previous),
            ],
        )?;
    }
    fetch(home, dir, upstream, target)?;
    run(
        home,
        "pointing the staging repo at the target",
        [
            git_dir.as_os_str(),
            arg("symbolic-ref"),
            arg("HEAD"),
            arg(&branch),
        ],
    )?;
    Ok(())
}

/// Makes the staging repo's `target` mirror `upstream`'s. Forced: staging
/// follows the target, whatever happened upstream or was pushed to it.
pub(crate) fn fetch(
    home: &Path,
    dir: &Path,
    upstream: &Upstream,
    target: &str,
) -> Result<(), GitError> {
    fetch_into(home, dir, upstream, target, &format!("refs/heads/{target}"))
}

/// As [`fetch`], into `dest` rather than staging's own `target`.
pub(crate) fn fetch_into(
    home: &Path,
    dir: &Path,
    upstream: &Upstream,
    target: &str,
    dest: &str,
) -> Result<(), GitError> {
    let mut git_dir = OsString::from("--git-dir=");
    git_dir.push(dir);
    let refspec = format!("+refs/heads/{target}:{dest}");
    let mut cmd = command::git(home);
    cmd.args([
        git_dir.as_os_str(),
        // A connection that stalls fails in half a minute rather than
        // holding up whatever waits on the fetch.
        OsStr::new("-c"),
        OsStr::new("http.lowSpeedLimit=1"),
        OsStr::new("-c"),
        OsStr::new("http.lowSpeedTime=30"),
        OsStr::new("fetch"),
        OsStr::new("--quiet"),
        OsStr::new("--no-tags"),
        OsStr::new("--no-write-fetch-head"),
        // A shallow upstream makes staging shallow too, instead of failing.
        OsStr::new("--update-shallow"),
        location(upstream),
        OsStr::new(&refspec),
    ]);
    authorize(&mut cmd, upstream);
    let doing = format!("fetching {target} from {}", describe(upstream));
    run_command(cmd, &doing)?;
    Ok(())
}

pub(crate) fn has(dir: &Path, target: &str) -> Result<bool, GitError> {
    if !dir.exists() {
        return Ok(false);
    }
    let failed = |e: &dyn std::fmt::Display| {
        GitError::Failed(format!("reading {} failed: {e}", dir.display()))
    };
    let repo = gix::open_opts(dir, gix::open::Options::isolated()).map_err(|e| failed(&e))?;
    let found = repo
        .try_find_reference(format!("refs/heads/{target}").as_str())
        .map_err(|e| failed(&e))?;
    Ok(found.is_some())
}
