//! Replaying a revision onto a moved base, for interdiffs.

use std::ffi::OsStr;

use mergegate_core::git::GitError;
use mergegate_core::interdiff::{InterdiffGit, Replay};
use mergegate_core::project::ProjectName;

use crate::HubGit;
use crate::command::{self, failed, output_of};

impl InterdiffGit for HubGit {
    /// `--merge-base` needs git 2.40.
    fn replay(
        &self,
        project: &ProjectName,
        base: &str,
        onto: &str,
        head: &str,
    ) -> Result<Replay, GitError> {
        let doing = "replaying a revision in the staging repo";
        let merge_base = format!("--merge-base={base}");
        let mut merge_tree = command::git(&self.home);
        merge_tree.args([
            self.git_dir(project).as_os_str(),
            OsStr::new("merge-tree"),
            OsStr::new("--write-tree"),
            OsStr::new("--name-only"),
            OsStr::new("--no-messages"),
            OsStr::new("-z"),
            OsStr::new(&merge_base),
            OsStr::new(onto),
            OsStr::new(head),
        ]);
        let output = output_of(merge_tree, doing)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut fields = stdout.split('\0').filter(|f| !f.is_empty());
        let tree = fields.next().map(str::trim).unwrap_or_default().to_owned();
        let mut conflicts: Vec<String> = match output.status.code() {
            Some(0) => Vec::new(),
            // A path is listed once per conflicting stage.
            Some(1) => fields.map(Into::into).collect(),
            _ => return Err(failed(doing, &output)),
        };
        conflicts.sort();
        conflicts.dedup();
        Ok(Replay { tree, conflicts })
    }
}
