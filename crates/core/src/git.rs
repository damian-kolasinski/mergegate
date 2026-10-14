//! The git port: the repositories on the hub's machine, and the hub's own
//! staging repos.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::project::ProjectName;

/// The oldest git with `merge-tree --write-tree`, which merging relies on.
pub const MIN_VERSION: (u32, u32) = (2, 38);

/// A repository as the hub sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// Canonical path of the main worktree, or of the git directory when the
    /// repo is bare. Every linked worktree of a repo has the same root.
    pub root: PathBuf,
    /// Local branch names, sorted.
    pub branches: Vec<String>,
    /// The branch checked out in the main worktree, if HEAD is on one.
    pub head: Option<String>,
    /// Its configured remotes, sorted by name.
    pub remotes: Vec<RepoRemote>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRemote {
    pub name: String,
    /// The fetch URL, as configured.
    pub url: String,
}

impl Repo {
    pub fn has_branch(&self, branch: &str) -> bool {
        self.branches.iter().any(|b| b == branch)
    }
}

/// Where staging takes a project's target from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Upstream {
    /// [`Repo::root`] of the operator's repo on the hub's machine.
    Repo(PathBuf),
    Remote(Remote),
}

/// A repository git reaches over HTTPS, as the hub's GitHub account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub url: String,
    /// The HTTP header that authorizes git at `url`, and nowhere else.
    pub auth: Secret,
}

/// A credential. It never shows in debug output, so it can't reach a log or
/// an error message by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(..)")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("{} is not inside a git repository", .0.display())]
    NotARepo(PathBuf),
    #[error("{0}")]
    Failed(String),
}

pub trait Git {
    /// The repository containing `path`. A linked worktree resolves to the
    /// repository that owns it.
    fn inspect(&self, path: &Path) -> Result<Repo, GitError>;

    /// Creates the project's staging repo if it doesn't exist and makes its
    /// `target` mirror `upstream`'s.
    fn sync_staging(
        &self,
        project: &ProjectName,
        upstream: &Upstream,
        target: &str,
    ) -> Result<(), GitError>;

    /// Whether the project's staging repo exists and has `target`.
    fn staging_has(&self, project: &ProjectName, target: &str) -> Result<bool, GitError>;

    /// What `git --version` prints.
    fn version(&self) -> Result<String, GitError>;
}

/// `(major, minor)` from `git --version` output such as
/// `git version 2.50.1 (Apple Git-155)`.
pub fn parse_version(output: &str) -> Option<(u32, u32)> {
    let version = output.trim().strip_prefix("git version ")?;
    let mut parts = version.split(|c: char| !c.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
