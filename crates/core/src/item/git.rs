use crate::git::{GitError, Remote, Upstream};
use crate::project::ProjectName;
use crate::triage::Triage;

/// What the item slice needs from the project's staging repo. Commits are
/// full hex object IDs.
pub trait ItemGit {
    /// Makes staging's `target` mirror `upstream`'s, leaving HEAD and every
    /// other ref alone: open items keep the target they were opened with.
    fn fetch_target(
        &self,
        project: &ProjectName,
        upstream: &Upstream,
        target: &str,
    ) -> Result<(), GitError>;

    /// Pushes staging's `head` to `branch` on `remote`, provided the remote
    /// has the branch where this staging last pushed it, or doesn't have it.
    /// `pushed` is the head of the open item whose pull request the branch
    /// carries: the lease when staging has no record, and a branch that's
    /// gone isn't created again under it. A remote already at `head` counts
    /// as pushed, so a submit that failed after its push can be retried.
    fn push_branch(
        &self,
        project: &ProjectName,
        remote: &Remote,
        branch: &str,
        head: &str,
        pushed: Option<&str>,
    ) -> Result<Pushed, GitError>;

    fn branch_head(&self, project: &ProjectName, branch: &str) -> Result<Option<String>, GitError>;

    /// `head` is a commit and `target` a branch.
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
    ) -> Result<Vec<ChangedFile>, GitError>;

    /// The subject of the first commit after `base` on the way to `head`.
    fn first_subject(
        &self,
        project: &ProjectName,
        base: &str,
        head: &str,
    ) -> Result<Option<String>, GitError>;

    fn pin(&self, project: &ProjectName, reference: &str, commit: &str) -> Result<(), GitError>;

    /// Deletes every ref under `prefix`, which ends in `/`. None is fine.
    fn unpin(&self, project: &ProjectName, prefix: &str) -> Result<(), GitError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pushed {
    Done,
    /// The remote has the branch at this commit, which the hub didn't push
    /// and the head doesn't contain.
    Moved(String),
    /// The remote no longer has the branch of an open pull request.
    Gone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub old_path: Option<String>,
    /// Object IDs before and after, none on the side where the file doesn't
    /// exist. A submodule's are commits.
    pub old_blob: Option<String>,
    pub new_blob: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub file: ChangedFile,
    /// Lines added and deleted; none when the change isn't text.
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    /// `linguist-generated` as the revision's base sets it.
    pub generated: bool,
    pub triage: Triage,
}

/// A full object ID: 40 (SHA-1) or 64 (SHA-256) lowercase hex digits.
pub fn valid_commit(id: &str) -> bool {
    matches!(id.len(), 40 | 64) && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
