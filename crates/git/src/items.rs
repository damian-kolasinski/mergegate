//! What the item slice reads and writes in a project's staging repo.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use mergegate_core::git::GitError;
use mergegate_core::item::{FileStat, ItemGit};
use mergegate_core::project::ProjectName;

use crate::command::{run, run_optional};
use crate::{HubGit, staging};

impl ItemGit for HubGit {
    fn fetch_target(
        &self,
        project: &ProjectName,
        upstream: &Path,
        target: &str,
    ) -> Result<(), GitError> {
        staging::fetch(&self.home, &self.staging(project), upstream, target)
    }

    fn branch_head(&self, project: &ProjectName, branch: &str) -> Result<Option<String>, GitError> {
        let dir = self.staging(project);
        let failed = |e: &dyn std::fmt::Display| {
            GitError::Failed(format!("reading {branch} in {} failed: {e}", dir.display()))
        };
        let repo = gix::open_opts(&dir, gix::open::Options::isolated()).map_err(|e| failed(&e))?;
        let Some(mut reference) = repo
            .try_find_reference(format!("refs/heads/{branch}").as_str())
            .map_err(|e| failed(&e))?
        else {
            return Ok(None);
        };
        let id = reference.peel_to_id().map_err(|e| failed(&e))?;
        Ok(Some(id.to_string()))
    }

    fn merge_base(
        &self,
        project: &ProjectName,
        head: &str,
        target: &str,
    ) -> Result<Option<String>, GitError> {
        let target = format!("refs/heads/{target}");
        let base = run_optional(
            &self.home,
            "finding the merge-base",
            [
                self.git_dir(project).as_os_str(),
                arg("merge-base"),
                arg(head),
                arg(&target),
            ],
        )?;
        Ok(base.map(|base| base.trim().to_owned()))
    }

    fn changed_files(
        &self,
        project: &ProjectName,
        base: &str,
        head: &str,
    ) -> Result<Vec<FileStat>, GitError> {
        let doing = "listing the changed files";
        let output = run(
            &self.home,
            doing,
            [
                self.git_dir(project).as_os_str(),
                arg("diff"),
                arg("--numstat"),
                arg("-z"),
                arg("-M"),
                arg("--no-ext-diff"),
                arg("--no-textconv"),
                arg(base),
                arg(head),
            ],
        )?;
        parse_numstat(&output)
            .ok_or_else(|| GitError::Failed(format!("{doing} failed: can't parse {output:?}")))
    }

    fn first_subject(
        &self,
        project: &ProjectName,
        base: &str,
        head: &str,
    ) -> Result<Option<String>, GitError> {
        let range = format!("{base}..{head}");
        let log = run(
            &self.home,
            "reading the commit subjects",
            [
                self.git_dir(project).as_os_str(),
                arg("log"),
                arg("--first-parent"),
                arg("--reverse"),
                arg("--format=%s"),
                arg(&range),
            ],
        )?;
        Ok(log.lines().next().map(str::to_owned))
    }

    fn pin(&self, project: &ProjectName, reference: &str, commit: &str) -> Result<(), GitError> {
        run(
            &self.home,
            &format!("pinning {reference}"),
            [
                self.git_dir(project).as_os_str(),
                arg("update-ref"),
                arg(reference),
                arg(commit),
            ],
        )?;
        Ok(())
    }
}

impl HubGit {
    fn git_dir(&self, project: &ProjectName) -> OsString {
        let mut git_dir = OsString::from("--git-dir=");
        git_dir.push(self.staging(project));
        git_dir
    }
}

fn arg(s: &str) -> &OsStr {
    OsStr::new(s)
}

/// Parses `git diff --numstat -z`: `added TAB deleted TAB path NUL` per file,
/// with `path` empty and followed by `old NUL new NUL` for a rename, and
/// `-` for both counts of a binary file.
fn parse_numstat(output: &str) -> Option<Vec<FileStat>> {
    let mut fields = output.split('\0');
    let mut files = Vec::new();
    while let Some(field) = fields.next().filter(|f| !f.is_empty()) {
        let mut parts = field.splitn(3, '\t');
        let additions = count(parts.next()?)?;
        let deletions = count(parts.next()?)?;
        let (path, old_path) = match parts.next()? {
            "" => {
                let old = fields.next()?;
                (fields.next()?, Some(old.to_owned()))
            }
            path => (path, None),
        };
        files.push(FileStat {
            path: path.to_owned(),
            old_path,
            additions,
            deletions,
        });
    }
    Some(files)
}

/// A line count, or `Some(None)` for a binary file's `-`.
fn count(field: &str) -> Option<Option<u64>> {
    match field {
        "-" => Some(None),
        n => n.parse().ok().map(Some),
    }
}

#[cfg(test)]
#[path = "items_tests.rs"]
mod tests;
