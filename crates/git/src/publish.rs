//! Pushing an item's branch from staging to its remote, as the `github`
//! backend publishes it.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Output;

use mergegate_core::git::{GitError, Remote};
use mergegate_core::item::Pushed;

use crate::command::{self, failed, output_of, run, run_command, run_optional};
use crate::remote::authorize_remote;

/// Leased, so the push only replaces the branch where mergegate last pushed
/// it: an agent's rebase goes through, anyone else's push to the remote
/// stops it until the agent builds on it, as pushing a head that contains
/// it loses nobody's work. Staging keeps that place under `refs/mergegate/published/`,
/// rather than taking it from the item, so a submit that failed after its
/// push, or a merge that left its branch behind, doesn't block the next;
/// `pushed`, the open item's head, stands in only when staging has lost it.
pub(crate) fn push_branch(
    home: &Path,
    staging: &Path,
    remote: &Remote,
    branch: &str,
    head: &str,
    pushed: Option<&str>,
) -> Result<Pushed, GitError> {
    let mut git_dir = OsString::from("--git-dir=");
    git_dir.push(staging);
    let reference = format!("refs/heads/{branch}");
    // Hex, so no branch's ref is a directory of another's (`a` and `a/b`).
    let published: String = branch.bytes().map(|b| format!("{b:02x}")).collect();
    let published = format!("refs/mergegate/published/{published}");
    let doing = format!("pushing {branch} to {}", remote.url);
    let push = |lease: Option<&str>| -> Result<Output, GitError> {
        let lease = format!(
            "--force-with-lease={reference}:{}",
            lease.unwrap_or_default()
        );
        let refspec = format!("{head}:{reference}");
        let mut cmd = command::git(home);
        cmd.args([
            git_dir.as_os_str(),
            OsStr::new("push"),
            OsStr::new("--porcelain"),
            OsStr::new("--no-verify"),
            OsStr::new(&lease),
            OsStr::new(&remote.url),
            OsStr::new(&refspec),
        ]);
        authorize_remote(&mut cmd, remote);
        output_of(cmd, &doing)
    };

    let last = run_optional(
        home,
        &format!("reading {published}"),
        [
            git_dir.as_os_str(),
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new("--quiet"),
            OsStr::new(&published),
        ],
    )?
    .map(|id| id.trim().to_owned());
    let mut lease = last.or(pushed.map(str::to_owned));
    loop {
        let output = push(lease.as_deref())?;
        if !output.status.success() {
            // Turned down: say where the branch is, unless it's turned down
            // for another reason, as a protected branch is.
            match remote_head(home, &git_dir, remote, &reference)? {
                Some(found) if found == head => {}
                // Under an open pull request, gone means closed on GitHub,
                // which this push can't reopen.
                None if pushed.is_some() => return Ok(Pushed::Gone),
                // Gone, as a merge leaves it: creating it again overwrites
                // nobody's work.
                None if lease.is_some() => {
                    lease = None;
                    continue;
                }
                Some(found) if Some(&found) != lease.as_ref() => {
                    if !contains(home, &git_dir, head, &found)? {
                        return Ok(Pushed::Moved(found));
                    }
                    lease = Some(found);
                    continue;
                }
                _ => return Err(failed(&doing, &output)),
            }
        }
        run(
            home,
            &format!("recording {published}"),
            [
                git_dir.as_os_str(),
                OsStr::new("update-ref"),
                OsStr::new(&published),
                OsStr::new(head),
            ],
        )?;
        return Ok(Pushed::Done);
    }
}

/// Whether `head` has `commit` in its history. Staging lacks `commit` unless
/// the agent fetched it and pushed it with `head`, which is all this asks.
fn contains(home: &Path, git_dir: &OsStr, head: &str, commit: &str) -> Result<bool, GitError> {
    let mut cmd = command::git(home);
    cmd.args([
        git_dir,
        OsStr::new("merge-base"),
        OsStr::new("--is-ancestor"),
        OsStr::new(commit),
        OsStr::new(head),
    ]);
    let output = output_of(cmd, &format!("checking {head} contains {commit}"))?;
    Ok(output.status.success())
}

fn remote_head(
    home: &Path,
    git_dir: &OsStr,
    remote: &Remote,
    reference: &str,
) -> Result<Option<String>, GitError> {
    let mut cmd = command::git(home);
    cmd.args([
        git_dir,
        OsStr::new("ls-remote"),
        OsStr::new("--refs"),
        OsStr::new(&remote.url),
        OsStr::new(reference),
    ]);
    authorize_remote(&mut cmd, remote);
    let listed = run_command(cmd, &format!("reading {reference} on {}", remote.url))?;
    // `<id>\t<ref>` per line; patterns match ref suffixes, so look for ours.
    Ok(listed.lines().find_map(|line| {
        let (id, name) = line.split_once('\t')?;
        (name == reference).then(|| id.to_owned())
    }))
}
