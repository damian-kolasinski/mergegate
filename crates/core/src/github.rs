//! The GitHub port: what the `github` backend needs from GitHub's API. Git
//! itself reaches GitHub through the git port, with a [`Remote`] this port
//! hands out.

mod detect;
mod repo;

use std::fmt;

pub use detect::{Detected, detect};
pub use repo::GithubRepo;

use crate::git::Remote;

/// A pull request the hub opened for an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pull {
    pub number: u64,
    /// Its page on GitHub.
    pub url: String,
}

/// Every call names the repository; the adapter authenticates as whoever the
/// operator is logged in as.
pub trait Github {
    /// Where git reaches `repo`, with the credentials it needs there.
    fn remote(&self, repo: &GithubRepo) -> Result<Remote, GithubError>;

    /// Fails unless `repo` exists and the operator's account can push to it.
    fn check_access(&self, repo: &GithubRepo) -> Result<(), GithubError>;

    /// The open pull request from `branch` of `repo` itself into `target`,
    /// if any.
    fn find_pull(
        &self,
        repo: &GithubRepo,
        branch: &str,
        target: &str,
    ) -> Result<Option<Pull>, GithubError>;

    /// Opens a pull request from `branch` into `target`, ready for review,
    /// with `title` and an empty body.
    fn open_pull(
        &self,
        repo: &GithubRepo,
        branch: &str,
        target: &str,
        title: &str,
    ) -> Result<Pull, GithubError>;

    fn retitle_pull(&self, repo: &GithubRepo, number: u64, title: &str) -> Result<(), GithubError>;

    /// Squash-merges the pull request, as long as its head is still `head`,
    /// into one commit titled `title` with an empty message, and returns the
    /// commit. One already merged at `head` returns its commit, so a retry
    /// after a merge the hub never recorded goes through.
    fn squash_merge(
        &self,
        repo: &GithubRepo,
        number: u64,
        title: &str,
        head: &str,
    ) -> Result<String, GithubError>;

    /// The commit the pull request was merged as, if it was merged at
    /// `head`.
    fn merged(
        &self,
        repo: &GithubRepo,
        number: u64,
        head: &str,
    ) -> Result<Option<String>, GithubError>;

    /// Closing a closed pull request is fine.
    fn close_pull(&self, repo: &GithubRepo, number: u64) -> Result<(), GithubError>;

    /// A branch that's already gone is fine.
    fn delete_branch(&self, repo: &GithubRepo, branch: &str) -> Result<(), GithubError>;
}

#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    /// The hub has no GitHub credentials to use, or GitHub rejected them.
    #[error("the hub can't sign in to GitHub: {0}")]
    NotLoggedIn(String),
    #[error("{0} doesn't exist on GitHub, or the account the hub signs in as can't push to it")]
    NoAccess(GithubRepo),
    /// GitHub understood the request and declined it; the message is its.
    #[error("GitHub refused: {0}")]
    Refused(String),
    #[error("{0}")]
    Failed(String),
}

impl fmt::Display for Pull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.number)
    }
}
