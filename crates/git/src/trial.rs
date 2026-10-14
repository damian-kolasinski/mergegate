//! Merging in staging without landing anything: the `github` backend's check
//! for conflicts, and the first step of the local backend's merge.

use std::ffi::OsStr;

use mergegate_core::git::{GitError, Upstream};
use mergegate_core::merge::{MergeGit, Trial};
use mergegate_core::project::ProjectName;

use crate::command::{self, failed, output_of, run};
use crate::{HubGit, staging};

pub(crate) enum Merged {
    Tree(String),
    Conflict(Vec<String>),
}

impl MergeGit for HubGit {
    fn trial_merge(
        &self,
        project: &ProjectName,
        upstream: &Upstream,
        target: &str,
        head: &str,
    ) -> Result<Trial, GitError> {
        staging::fetch(&self.home, &self.staging(project), upstream, target)?;
        let target_commit = self.rev_parse(project, &format!("refs/heads/{target}^{{commit}}"))?;
        Ok(match self.merge_tree(project, &target_commit, head)? {
            Merged::Tree(_) => Trial::Clean,
            Merged::Conflict(paths) => Trial::Conflict {
                target_commit,
                paths,
            },
        })
    }
}

impl HubGit {
    /// The tree of `head` merged into `target_commit`, written to staging's
    /// objects but to no ref.
    pub(crate) fn merge_tree(
        &self,
        project: &ProjectName,
        target_commit: &str,
        head: &str,
    ) -> Result<Merged, GitError> {
        let doing = "merging in the staging repo";
        let mut merge_tree = command::git(&self.home);
        merge_tree.args([
            self.git_dir(project).as_os_str(),
            OsStr::new("merge-tree"),
            OsStr::new("--write-tree"),
            OsStr::new("--name-only"),
            OsStr::new("--no-messages"),
            OsStr::new("-z"),
            OsStr::new(target_commit),
            OsStr::new(head),
        ]);
        let output = output_of(merge_tree, doing)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut fields = stdout.split('\0').filter(|f| !f.is_empty());
        let tree = fields.next().map(str::trim).unwrap_or_default().to_owned();
        match output.status.code() {
            Some(0) => Ok(Merged::Tree(tree)),
            Some(1) => Ok(Merged::Conflict(fields.map(Into::into).collect())),
            _ => Err(failed(doing, &output)),
        }
    }

    pub(crate) fn rev_parse(&self, project: &ProjectName, rev: &str) -> Result<String, GitError> {
        let found = run(
            &self.home,
            &format!("reading {rev}"),
            [
                self.git_dir(project).as_os_str(),
                OsStr::new("rev-parse"),
                OsStr::new("--verify"),
                OsStr::new(rev),
            ],
        )?;
        Ok(found.trim().to_owned())
    }
}
