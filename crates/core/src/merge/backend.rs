use crate::git::GitError;
use crate::item::{Item, ItemId};
use crate::project::{Backend, Project};

/// Where approved work lands: the operator's repo for `local`, the pull
/// request for `github`. An item merges only through the backend it was
/// opened with.
pub trait MergeBackend {
    fn kind(&self) -> Backend;

    /// Lands `item`'s newest revision on its target as one commit, with
    /// `message` (a subject line, then a blank line and the body, if any) and
    /// the target as its only parent. The local backend makes
    /// the operator its author; GitHub makes it the operator's account. A
    /// merge that went through before the hub could record it lands again as
    /// the commit it made.
    fn land(&self, project: &Project, item: &Item, message: &str) -> Result<Landing, GitError>;

    /// Points the review of `child`, stacked on an item that just landed, at
    /// its target, before the parent's branch goes: GitHub retargets its pull
    /// request, which was based on that branch.
    fn restack(&self, _project: &Project, _child: &Item) -> Result<(), GitError> {
        Ok(())
    }

    /// Clears up after `item` landed, once nothing stacked on it needs its
    /// branch: GitHub deletes the branch. A failure only leaves clutter.
    fn tidy(&self, _project: &Project, _item: &Item) {}
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
    /// Not tried: the item lands right after `parent`, which it's stacked on
    /// and which hasn't yet. No backend returns it.
    Waiting {
        parent: ItemId,
    },
}
