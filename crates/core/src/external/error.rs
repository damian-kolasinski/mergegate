use crate::git::GitError;
use crate::github::GithubError;
use crate::item::ItemId;
use crate::review::{CommentId, ReviewError};
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum ExternalError {
    /// The item takes no more feedback.
    #[error(transparent)]
    Review(#[from] ReviewError),
    #[error("{0} has no pull request on GitHub, so it has no comments there")]
    NotGithub(ItemId),
    #[error("no external comment {0}")]
    NoComment(CommentId),
    #[error("the agent's latest reply on {0} doesn't reject it")]
    NotRejected(CommentId),
    #[error(transparent)]
    Github(#[from] GithubError),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Store(#[from] StoreError),
}
