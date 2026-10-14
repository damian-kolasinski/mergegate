//! Landing a `github` item: GitHub squash-merges its pull request.

use super::{Landing, MergeBackend, MergeGit, Trial};
use crate::git::{GitError, Upstream};
use crate::github::Github;
use crate::item::{Item, ItemBackend, ItemGit};
use crate::project::{Backend, Project};

pub struct GithubBackend<'a, G, H> {
    git: &'a G,
    github: &'a H,
}

impl<'a, G, H> GithubBackend<'a, G, H> {
    pub fn new(git: &'a G, github: &'a H) -> Self {
        Self { git, github }
    }
}

impl<G: ItemGit + MergeGit, H: Github> MergeBackend for GithubBackend<'_, G, H> {
    fn kind(&self) -> Backend {
        Backend::Github
    }

    /// Conflicts are found in staging first, so they reach the agent with
    /// their paths, as the local backend reports them. GitHub then merges
    /// only if the pull request's head is still the approved revision, and
    /// anything it refuses holds the merge with its reason.
    fn land(&self, project: &Project, item: &Item, message: &str) -> Result<Landing, GitError> {
        let held = |reason: String| Ok(Landing::Held { reason });
        let ItemBackend::Github { repo, pull } = &item.backend else {
            return held(format!("{} has no pull request on GitHub", item.id));
        };
        let upstream = match self.github.remote(repo) {
            Ok(remote) => Upstream::Remote(remote),
            Err(e) => return held(e.to_string()),
        };
        let head = &item.revision.head;
        let trial = self
            .git
            .trial_merge(&project.name, &upstream, &item.target, head)?;
        let merged = match trial {
            Trial::Clean => self.github.squash_merge(repo, pull.number, message, head),
            // A merge the hub never recorded is on the target now, where
            // later commits may conflict with it. Unless GitHub says so, the
            // conflict stands, so the agent still hears about it.
            Trial::Conflict {
                target_commit,
                paths,
            } => match self.github.merged(repo, pull.number, head) {
                Ok(Some(commit)) => Ok(commit),
                Ok(None) | Err(_) => {
                    return Ok(Landing::Conflict {
                        target_commit,
                        paths,
                    });
                }
            },
        };
        let commit = match merged {
            Ok(commit) => commit,
            Err(e) => return held(e.to_string()),
        };
        // The merge is done on GitHub whatever these do: staging catches up
        // on its next fetch, and a branch left behind only clutters GitHub.
        let _ = self
            .git
            .fetch_target(&project.name, &upstream, &item.target);
        let _ = self.github.delete_branch(repo, &item.branch);
        Ok(Landing::Merged { commit })
    }
}

#[cfg(test)]
#[path = "github_tests.rs"]
mod tests;
