//! Publishing a `github` item's revision: its branch and pull request.

use super::{Item, ItemBackend, ItemError, ItemGit, ItemId, Pushed};
use crate::git::Remote;
use crate::github::{Github, GithubRepo, Pull};
use crate::project::ProjectName;

pub(super) struct Revision<'a> {
    /// The item's, or the one it will get.
    pub id: &'a ItemId,
    pub branch: &'a str,
    pub head: &'a str,
    pub target: &'a str,
    pub title: &'a str,
    pub guide: Option<&'a str>,
}

/// Pushes `revision.head` to the item's branch on GitHub, then opens its
/// pull request or brings its title and description up to date. The push
/// leases the branch from where the hub last pushed it, so work pushed to
/// GitHub by anyone else is never overwritten. Each step tolerates a retry
/// after a submit that failed partway.
///
/// `existing` is the open item, with the guide its pull request describes.
/// The pull request is edited only when its title or guide changes, so an
/// edit made on GitHub stands until then.
pub(super) fn publish(
    git: &impl ItemGit,
    github: &impl Github,
    project: &ProjectName,
    (repo, remote): (&GithubRepo, &Remote),
    existing: Option<(&Item, Option<&str>)>,
    revision: Revision,
) -> Result<Pull, ItemError> {
    let Revision {
        id,
        branch,
        head,
        target,
        title,
        guide,
    } = revision;
    let open = existing.and_then(|(item, described)| match &item.backend {
        ItemBackend::Github { pull, .. } => Some((item, described, pull)),
        ItemBackend::Local => None,
    });
    let pushed = open.map(|(item, ..)| item.revision.head.as_str());
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
    let body = description(id, guide);
    if let Some((item, described, pull)) = open {
        if item.title != title || described != guide {
            github.edit_pull(repo, pull.number, title, &body)?;
        }
        return Ok(pull.clone());
    }
    match github.find_pull(repo, branch, target)? {
        Some(pull) => {
            github.edit_pull(repo, pull.number, title, &body)?;
            Ok(pull)
        }
        None => Ok(github.open_pull(repo, branch, target, title, &body)?),
    }
}

/// The pull request's description: the item it's reviewed as, then the
/// agent's guide, so a reader on GitHub learns what the change is for.
pub(super) fn description(id: &ItemId, guide: Option<&str>) -> String {
    let named = format!("Reviewed in mergegate as {id}.");
    match guide.map(str::trim).filter(|guide| !guide.is_empty()) {
        Some(guide) => format!("{named}\n\n{guide}"),
        None => named,
    }
}
