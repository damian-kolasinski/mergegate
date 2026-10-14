use super::ItemId;
use crate::git::GitError;
use crate::github::GithubError;
use crate::project::ProjectError;
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum ItemError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("no item {0}")]
    NoItem(ItemId),
    #[error("{0} is closed")]
    Closed(ItemId),
    #[error("{0:?} is not a valid branch name")]
    InvalidBranch(String),
    #[error("{0:?} is not a full commit ID")]
    InvalidCommit(String),
    #[error("{branch} is the target branch; submit work from its own branch")]
    TargetBranch { branch: String },
    #[error("the hub has {branch} at {found}, not {head}; push it first")]
    NotPushed {
        branch: String,
        head: String,
        found: String,
    },
    #[error("the hub has no branch {0}; push it first")]
    NoBranch(String),
    #[error("{branch} shares no history with {target}")]
    Unrelated { branch: String, target: String },
    #[error("{branch} has no commits that aren't on {target}")]
    NothingToSubmit { branch: String, target: String },
    /// `found` is where GitHub has the branch.
    #[error("GitHub has {branch} at {found}, which mergegate didn't push")]
    BranchMoved { branch: String, found: String },
    #[error("GitHub no longer has {branch}, which mergegate pushed for an open pull request")]
    BranchGone { branch: String },
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Github(#[from] GithubError),
    #[error(transparent)]
    Store(#[from] StoreError),
}
