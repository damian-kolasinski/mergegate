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
        // A stacked item's pull request may still be based on its parent's
        // branch, and GitHub merges into whatever it's based on.
        if item.parent.is_some()
            && let Err(e) = self.github.retarget(repo, pull.number, &item.target)
        {
            return held(e.to_string());
        }
        let head = &item.revision.head;
        // Its head still holds the parent revision it's built on, which
        // GitHub merges from the old merge-base, so a parent that changed
        // before landing must not come back through it.
        let base = item.parent.as_ref().map(|_| item.revision.base.as_str());
        let trial = self
            .git
            .trial_merge(&project.name, &upstream, &item.target, head, base)?;
        // GitHub takes the subject and the body apart.
        let (title, body) = message.split_once("\n\n").unwrap_or((message, ""));
        let merged = match trial {
            Trial::Clean => self
                .github
                .squash_merge(repo, pull.number, title, body, head),
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
        // The merge is done on GitHub whatever this does: staging catches up
        // on its next fetch.
        let _ = self
            .git
            .fetch_target(&project.name, &upstream, &item.target);
        Ok(Landing::Merged { commit })
    }

    fn restack(&self, _project: &Project, child: &Item) -> Result<(), GitError> {
        let ItemBackend::Github { repo, pull } = &child.backend else {
            return Ok(());
        };
        self.github
            .retarget(repo, pull.number, &child.target)
            .map_err(|e| GitError::Failed(format!("GitHub didn't retarget {}: {e}", child.id)))
    }

    /// A branch left behind only clutters GitHub.
    fn tidy(&self, _project: &Project, item: &Item) {
        if let ItemBackend::Github { repo, .. } = &item.backend {
            let _ = self.github.delete_branch(repo, &item.branch);
        }
    }
}

#[cfg(test)]
#[path = "github_tests.rs"]
mod tests;
