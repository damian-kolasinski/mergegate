//! Test-merging in staging for early conflict detection, and the watch's
//! own fetch of each target.

use std::ffi::OsStr;

use mergegate_core::conflict::ConflictGit;
use mergegate_core::git::{GitError, Upstream};
use mergegate_core::item::ItemGit;
use mergegate_core::project::ProjectName;

use crate::command::{run, run_optional};
use crate::trial::Merged;
use crate::{HubGit, staging};

impl ConflictGit for HubGit {
    fn conflicts(
        &self,
        project: &ProjectName,
        ours: &str,
        theirs: &str,
    ) -> Result<Vec<String>, GitError> {
        Ok(match self.merge_tree(project, ours, theirs, None)? {
            Merged::Tree(_) => Vec::new(),
            Merged::Conflict(paths) => paths,
        })
    }

    fn fetch_watched(
        &self,
        project: &ProjectName,
        upstream: &Upstream,
        target: &str,
    ) -> Result<String, GitError> {
        // Under the hidden `refs/mergegate`, so workers never fetch it.
        let watched = format!("refs/mergegate/watch/{target}");
        let staging = self.staging(project);
        staging::fetch_into(&self.home, &staging, upstream, target, &watched)?;
        let commit = run(
            &self.home,
            &format!("reading {watched}"),
            [
                self.git_dir(project).as_os_str(),
                arg("rev-parse"),
                arg("--verify"),
                arg(&format!("{watched}^{{commit}}")),
            ],
        )?;
        Ok(commit.trim().to_owned())
    }

    fn advance(&self, project: &ProjectName, target: &str, commit: &str) -> Result<(), GitError> {
        let current = self.branch_head(project, target)?;
        if let Some(current) = &current {
            if current == commit {
                return Ok(());
            }
            let ancestor = run_optional(
                &self.home,
                "checking the target fast-forwards",
                [
                    self.git_dir(project).as_os_str(),
                    arg("merge-base"),
                    arg("--is-ancestor"),
                    arg(current),
                    arg(commit),
                ],
            )?;
            if ancestor.is_none() {
                return Ok(());
            }
        }
        let branch = format!("refs/heads/{target}");
        let git_dir = self.git_dir(project);
        let mut args = vec![
            git_dir.as_os_str(),
            arg("update-ref"),
            arg(&branch),
            arg(commit),
        ];
        // Only from what was checked; the store is held, so nothing moves it
        // in between, but a stale read fails rather than rewinds.
        if let Some(current) = &current {
            args.push(arg(current));
        }
        run(&self.home, &format!("moving {branch}"), args)?;
        Ok(())
    }
}

fn arg(s: &str) -> &OsStr {
    OsStr::new(s)
}
