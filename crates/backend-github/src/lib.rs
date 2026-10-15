//! GitHub, over its REST API, as the account the operator's `gh` is logged
//! in to. The token never leaves memory: it's read from `gh` when first
//! needed, and again whenever GitHub turns it down.

mod api;
mod comments;
mod pulls;
mod repo;
mod token;

use std::path::PathBuf;

use mergegate_core::git::Remote;
use mergegate_core::github::{Github, GithubError, GithubRepo, Pull, PullComment, PullCommentKind};

use api::Api;

pub struct GithubClient {
    api: Api,
    comments: comments::Cache,
    /// Where git reaches repositories, as in `https://github.com`.
    web: String,
}

impl GithubClient {
    /// `api` as in `https://api.github.com`, both URLs without a trailing
    /// slash; `gh` is the GitHub CLI to take the token from.
    pub fn new(api: String, web: String, gh: PathBuf) -> Self {
        Self {
            api: Api::new(api, gh),
            comments: comments::Cache::default(),
            web,
        }
    }

    /// github.com through the `gh` on `PATH`, unless `MG_GITHUB_API_URL` and
    /// `MG_GITHUB_URL` point elsewhere, as tests point them at a fake.
    pub fn from_env() -> Self {
        let var = |name, default: &str| {
            std::env::var(name)
                .ok()
                .filter(|url| !url.is_empty())
                .unwrap_or_else(|| default.into())
                .trim_end_matches('/')
                .to_owned()
        };
        Self::new(
            var("MG_GITHUB_API_URL", "https://api.github.com"),
            var("MG_GITHUB_URL", "https://github.com"),
            "gh".into(),
        )
    }
}

impl Github for GithubClient {
    fn remote(&self, repo: &GithubRepo) -> Result<Remote, GithubError> {
        repo::remote(&self.api, &self.web, repo)
    }

    fn check_access(&self, repo: &GithubRepo) -> Result<(), GithubError> {
        repo::check_access(&self.api, repo)
    }

    fn find_pull(
        &self,
        repo: &GithubRepo,
        branch: &str,
        target: &str,
    ) -> Result<Option<Pull>, GithubError> {
        pulls::find(&self.api, repo, branch, target)
    }

    fn open_pull(
        &self,
        repo: &GithubRepo,
        branch: &str,
        target: &str,
        title: &str,
        body: &str,
    ) -> Result<Pull, GithubError> {
        pulls::open(&self.api, repo, branch, target, title, body)
    }

    fn pull_body(&self, repo: &GithubRepo, number: u64) -> Result<String, GithubError> {
        pulls::body(&self.api, repo, number)
    }

    fn edit_pull(
        &self,
        repo: &GithubRepo,
        number: u64,
        title: &str,
        body: Option<&str>,
    ) -> Result<(), GithubError> {
        pulls::edit(&self.api, repo, number, title, body)
    }

    fn squash_merge(
        &self,
        repo: &GithubRepo,
        number: u64,
        title: &str,
        message: &str,
        head: &str,
    ) -> Result<String, GithubError> {
        pulls::squash_merge(&self.api, repo, number, title, message, head)
    }

    fn merged(
        &self,
        repo: &GithubRepo,
        number: u64,
        head: &str,
    ) -> Result<Option<String>, GithubError> {
        pulls::merged(&self.api, repo, number, head)
    }

    fn close_pull(&self, repo: &GithubRepo, number: u64) -> Result<(), GithubError> {
        pulls::close(&self.api, repo, number)
    }

    fn delete_branch(&self, repo: &GithubRepo, branch: &str) -> Result<(), GithubError> {
        repo::delete_branch(&self.api, repo, branch)
    }

    fn viewer(&self) -> Result<String, GithubError> {
        comments::viewer(&self.api, &self.comments)
    }

    fn pull_comments(
        &self,
        repo: &GithubRepo,
        number: u64,
        fresh: bool,
    ) -> Result<Vec<PullComment>, GithubError> {
        comments::of_pull(&self.api, &self.comments, repo, number, fresh)
    }

    fn pull_comment(
        &self,
        repo: &GithubRepo,
        number: u64,
        kind: PullCommentKind,
        id: u64,
    ) -> Result<Option<PullComment>, GithubError> {
        comments::one(&self.api, repo, number, kind, id)
    }
}
