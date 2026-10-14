//! Publishing a `github` item's revision: its branch and pull request.

use super::{Item, ItemBackend, ItemError, ItemGit, Pushed};
use crate::git::Remote;
use crate::github::{Github, GithubRepo, Pull};
use crate::project::ProjectName;

pub(super) struct Revision<'a> {
    pub branch: &'a str,
    pub head: &'a str,
    pub target: &'a str,
    pub title: &'a str,
}

/// Pushes `revision.head` to the item's branch on GitHub, then opens its
/// pull request or retitles it. The push leases the branch from where the
/// hub last pushed it, so work pushed to GitHub by anyone else is never
/// overwritten. Each step tolerates a retry after a submit that failed
/// partway.
pub(super) fn publish(
    git: &impl ItemGit,
    github: &impl Github,
    project: &ProjectName,
    (repo, remote): (&GithubRepo, &Remote),
    existing: Option<&Item>,
    revision: Revision,
) -> Result<Pull, ItemError> {
    let Revision {
        branch,
        head,
        target,
        title,
    } = revision;
    let open = existing.and_then(|item| match &item.backend {
        ItemBackend::Github { pull } => Some((item, pull)),
        ItemBackend::Local => None,
    });
    let pushed = open.map(|(item, _)| item.revision.head.as_str());
    match git.push_branch(project, remote, branch, head, pushed)? {
        Pushed::Done => {}
        Pushed::Moved(found) => {
            return Err(ItemError::BranchMoved {
                branch: branch.into(),
                found,
            });
        }
        Pushed::Gone => {
            return Err(ItemError::BranchGone {
                branch: branch.into(),
            });
        }
    }
    if let Some((item, pull)) = open {
        if item.title != title {
            github.retitle_pull(repo, pull.number, title)?;
        }
        return Ok(pull.clone());
    }
    match github.find_pull(repo, branch, target)? {
        Some(pull) => {
            github.retitle_pull(repo, pull.number, title)?;
            Ok(pull)
        }
        None => Ok(github.open_pull(repo, branch, target, title)?),
    }
}
