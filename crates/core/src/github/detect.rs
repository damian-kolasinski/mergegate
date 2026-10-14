//! Which GitHub repository a local clone publishes to.

use super::GithubRepo;
use crate::git::RepoRemote;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detected {
    None,
    One(GithubRepo),
    /// Several remotes point at different GitHub repositories and none is
    /// `origin`, sorted.
    Several(Vec<GithubRepo>),
}

/// `origin` if it's on GitHub, else the only GitHub repository the remotes
/// point at.
pub fn detect(remotes: &[RepoRemote]) -> Detected {
    let on_github = |remote: &RepoRemote| GithubRepo::from_url(&remote.url);
    if let Some(origin) = remotes
        .iter()
        .find(|r| r.name == "origin")
        .and_then(on_github)
    {
        return Detected::One(origin);
    }
    let mut repos: Vec<GithubRepo> = remotes.iter().filter_map(on_github).collect();
    repos.sort();
    repos.dedup();
    match repos.len() {
        0 => Detected::None,
        1 => Detected::One(repos.remove(0)),
        _ => Detected::Several(repos),
    }
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
