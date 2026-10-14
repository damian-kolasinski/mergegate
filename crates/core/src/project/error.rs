use std::path::PathBuf;

use super::{NameError, ProjectName};
use crate::git::GitError;
use crate::github::GithubError;
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("{} is not inside a git repository", .0.display())]
    NotARepo(PathBuf),
    #[error("{} has no branches yet", .0.display())]
    NoCommits(PathBuf),
    #[error("can't derive a project name from {}", .0.display())]
    NoName(PathBuf),
    #[error(transparent)]
    InvalidName(#[from] NameError),
    #[error("a project named {0} already exists")]
    NameTaken(ProjectName),
    #[error("{} is already registered as project {project}", upstream.display())]
    AlreadyRegistered {
        project: ProjectName,
        upstream: PathBuf,
    },
    #[error("{} has no branch {branch}", upstream.display())]
    UnknownBranch { branch: String, upstream: PathBuf },
    #[error("{branch} is the branch of open item {item}, so it can't be the target")]
    BranchUnderReview { branch: String, item: String },
    #[error("found no GitHub repository among the remotes of {}", .0.display())]
    NoGithubRepo(PathBuf),
    #[error("project {0} has no GitHub repository set")]
    NoGithubSet(ProjectName),
    #[error("no project named {0}")]
    NotFound(String),
    #[error("no project is registered for the repo at {}", .0.display())]
    NoProjectAt(PathBuf),
    #[error("unknown setting {0}; settings are {keys}", keys = super::KEYS.join(", "))]
    UnknownSetting(String),
    #[error("{key} must be one of {expected}, got {value:?}")]
    InvalidValue {
        key: &'static str,
        value: String,
        expected: &'static str,
    },
    #[error(transparent)]
    Git(GitError),
    #[error(transparent)]
    Github(#[from] GithubError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<GitError> for ProjectError {
    fn from(e: GitError) -> Self {
        match e {
            GitError::NotARepo(path) => Self::NotARepo(path),
            e => Self::Git(e),
        }
    }
}
