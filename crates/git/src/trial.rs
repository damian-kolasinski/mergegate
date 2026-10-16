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
        base: Option<&str>,
    ) -> Result<Trial, GitError> {
        staging::fetch(&self.home, &self.staging(project), upstream, target)?;
        let target_commit = self.rev_parse(project, &format!("refs/heads/{target}^{{commit}}"))?;
        let conflict = |paths| Trial::Conflict {
            target_commit: target_commit.clone(),
            paths,
        };
        let plain = match self.merge_tree(project, &target_commit, head, None)? {
            Merged::Tree(tree) => tree,
            Merged::Conflict(paths) => return Ok(conflict(paths)),
        };
        let Some(base) = base else {
            return Ok(Trial::Clean);
        };
        Ok(
            match self.merge_tree(project, &target_commit, head, Some(base))? {
                Merged::Tree(own) if own == plain => Trial::Clean,
                Merged::Tree(own) => conflict(self.changed(project, &own, &plain)?),
                Merged::Conflict(paths) => conflict(paths),
            },
        )
    }
}

impl HubGit {
    /// The tree of `head` merged into `target_commit`, written to staging's
    /// objects but to no ref. From `base` when given, as a stacked item's
    /// own diff applies from its parent revision; else from their merge-base.
    pub(crate) fn merge_tree(
        &self,
        project: &ProjectName,
        target_commit: &str,
        head: &str,
        base: Option<&str>,
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
        ]);
        let base = base.map(|base| format!("--merge-base={base}"));
        merge_tree.args(base.as_deref().map(OsStr::new));
        merge_tree.args([OsStr::new(target_commit), OsStr::new(head)]);
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

    /// The paths whose content differs between two trees.
    fn changed(&self, project: &ProjectName, a: &str, b: &str) -> Result<Vec<String>, GitError> {
        let names = run(
            &self.home,
            "comparing merges in the staging repo",
            [
                self.git_dir(project).as_os_str(),
                OsStr::new("diff-tree"),
                OsStr::new("-r"),
                OsStr::new("--name-only"),
                OsStr::new("-z"),
                OsStr::new(a),
                OsStr::new(b),
            ],
        )?;
        Ok(names
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(Into::into)
            .collect())
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
