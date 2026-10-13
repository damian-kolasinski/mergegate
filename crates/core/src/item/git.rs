use std::path::Path;

use crate::git::GitError;
use crate::project::ProjectName;

/// What the item slice needs from the project's staging repo. Commits are
/// full hex object IDs.
pub trait ItemGit {
    /// Makes staging's `target` mirror `upstream`'s, leaving HEAD and every
    /// other ref alone: open items keep the target they were opened with.
    fn fetch_target(
        &self,
        project: &ProjectName,
        upstream: &Path,
        target: &str,
    ) -> Result<(), GitError>;

    /// The commit `branch` points at in staging, if the branch exists.
    fn branch_head(&self, project: &ProjectName, branch: &str) -> Result<Option<String>, GitError>;

    /// The best common ancestor of commit `head` and branch `target`, if
    /// they share any history.
    fn merge_base(
        &self,
        project: &ProjectName,
        head: &str,
        target: &str,
    ) -> Result<Option<String>, GitError>;

    /// Files changed from `base` to `head`, with renames detected, in path
    /// order.
    fn changed_files(
        &self,
        project: &ProjectName,
        base: &str,
        head: &str,
    ) -> Result<Vec<FileStat>, GitError>;

    /// The subject of the first commit after `base` on the way to `head`.
    fn first_subject(
        &self,
        project: &ProjectName,
        base: &str,
        head: &str,
    ) -> Result<Option<String>, GitError>;

    /// Points `reference` at `commit`, creating or moving it.
    fn pin(&self, project: &ProjectName, reference: &str, commit: &str) -> Result<(), GitError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub path: String,
    /// The path before a rename or copy.
    pub old_path: Option<String>,
    /// Lines added and deleted; none for a binary file.
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
}

/// Whether `id` is a full object ID: 40 (SHA-1) or 64 (SHA-256) lowercase
/// hex digits.
pub fn valid_commit(id: &str) -> bool {
    matches!(id.len(), 40 | 64) && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
