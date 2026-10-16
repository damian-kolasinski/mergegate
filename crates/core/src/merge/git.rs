use crate::git::{GitError, Upstream};
use crate::project::ProjectName;

/// What landing through a backend that merges elsewhere needs from staging.
pub trait MergeGit {
    /// Fetches `target` from `upstream`, then merges `head` into it without
    /// writing a commit or moving a ref. With `base`, as a stacked item's
    /// own diff applies from its parent revision, it's clean only if the
    /// merge from their merge-base, which is all GitHub does, gives the same
    /// tree: else GitHub would land changes that diff doesn't hold.
    fn trial_merge(
        &self,
        project: &ProjectName,
        upstream: &Upstream,
        target: &str,
        head: &str,
        base: Option<&str>,
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
