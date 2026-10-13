use crate::git::GitError;
use crate::project::ProjectError;
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum ItemError {
    #[error(transparent)]
    Project(#[from] ProjectError),
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
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Store(#[from] StoreError),
}
