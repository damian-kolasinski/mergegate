use crate::git::{GitError, Upstream};
use crate::project::ProjectName;

/// What landing through a backend that merges elsewhere needs from staging.
pub trait MergeGit {
    /// Fetches `target` from `upstream`, then merges `head` into it without
    /// writing a commit or moving a ref.
    fn trial_merge(
        &self,
        project: &ProjectName,
        upstream: &Upstream,
        target: &str,
        head: &str,
    ) -> Result<Trial, GitError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trial {
    Clean,
    Conflict {
        target_commit: String,
        paths: Vec<String>,
    },
}
