use crate::git::GitError;
use crate::item::Item;
use crate::project::{Backend, Project};

/// Where approved work lands: the operator's repo for `local`, GitHub from
/// PR 9. An item merges only through the backend it was opened with.
pub trait MergeBackend {
    fn kind(&self) -> Backend;

    /// Lands `item`'s newest revision on its target as one commit, with
    /// `message`, the target as its only parent and the operator as author.
    /// A tree already on the target lands as the target's commit: that's a
    /// merge whose push went through before the hub could record it.
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
    /// their checkout of it has changes; `reason` is git's.
    Held {
        reason: String,
    },
}
