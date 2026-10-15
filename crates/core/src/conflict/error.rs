use crate::git::GitError;
use crate::github::GithubError;
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum ConflictError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Github(#[from] GithubError),
    #[error(transparent)]
    Store(#[from] StoreError),
}
