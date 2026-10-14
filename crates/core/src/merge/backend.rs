use crate::git::GitError;
use crate::item::Item;
use crate::project::{Backend, Project};

/// Where approved work lands: the operator's repo for `local`, the pull
/// request for `github`. An item merges only through the backend it was
/// opened with.
pub trait MergeBackend {
    fn kind(&self) -> Backend;

    /// Lands `item`'s newest revision on its target as one commit, with
    /// `message` and the target as its only parent. The local backend makes
    /// the operator its author; GitHub makes it the operator's account. A
    /// merge that went through before the hub could record it lands again as
    /// the commit it made.
    fn land(&self, project: &Project, item: &Item, message: &str) -> Result<Landing, GitError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landing {
    Merged {
        commit: String,
    },
    /// The revision and the target at `target_commit` change these paths
    /// both, so its agent rebases.
    Conflict {
        target_commit: String,
        paths: Vec<String>,
    },
    /// The target can't take the commit until the operator acts, as when
    /// their checkout of it has changes or GitHub refuses the merge;
    /// `reason` is git's or GitHub's.
    Held {
        reason: String,
    },
}
