//! Each project's staging repo: a bare repo the hub owns, where items are
//! merged and checked before anything touches the operator's repo.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use mergegate_core::git::GitError;

use crate::command::run;

pub(crate) fn sync(home: &Path, dir: &Path, upstream: &Path, target: &str) -> Result<(), GitError> {
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| GitError::Failed(format!("creating {} failed: {e}", parent.display())))?;
    }
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
    // Forced: staging mirrors the target, whatever happened upstream.
    let refspec = format!("+{branch}:{branch}");
    run(
        home,
        &format!("fetching {target} from {}", upstream.display()),
        [
            git_dir.as_os_str(),
            arg("fetch"),
            arg("--quiet"),
            arg("--no-tags"),
            arg("--no-write-fetch-head"),
            upstream.as_os_str(),
            arg(&refspec),
        ],
    )?;
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
