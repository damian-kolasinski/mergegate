//! Publishing a `github` item's revision: its branch and pull request.

use super::{Item, ItemBackend, ItemError, ItemGit, ItemId, Pushed};
use crate::git::Remote;
use crate::github::{Github, GithubError, GithubRepo, Pull};
use crate::project::ProjectName;
use crate::triage::Guide;

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
/// The pull request is retitled only when its title changes, and described
/// again only when its guide does, never over a description written on
/// GitHub.
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
    if let Some((item, described, pull)) = open {
        let body = (described != guide).then(|| description(id, guide));
        if item.title != title || body.is_some() {
            describe(github, repo, pull.number, title, body.as_deref())?;
        }
        return Ok(pull.clone());
    }
    let body = description(id, guide);
    match github.find_pull(repo, branch, target)? {
        Some(pull) => {
            describe(github, repo, pull.number, title, Some(&body))?;
            Ok(pull)
        }
        None => Ok(github.open_pull(repo, branch, target, title, &body)?),
    }
}

/// How every description the hub writes starts, which tells it apart from
/// one written on GitHub.
const NAMED: &str = "Reviewed in mergegate as";

/// GitHub refuses a description over 65,536 characters. No character takes
/// fewer bytes than it counts for, so one this many bytes long always fits.
const MAX_BODY: usize = 65_000;

/// Retitles the pull request, and describes it with `body` unless its
/// description was written on GitHub: that one stands.
pub(super) fn describe(
    github: &impl Github,
    repo: &GithubRepo,
    number: u64,
    title: &str,
    body: Option<&str>,
) -> Result<(), GithubError> {
    let body = match body {
        Some(body) => {
            let current = github.pull_body(repo, number)?;
            (current.trim().is_empty() || current.starts_with(NAMED)).then_some(body)
        }
        None => None,
    };
    github.edit_pull(repo, number, title, body)
}

/// The pull request's description: the item it's reviewed as, then the
/// agent's guide, so a reader on GitHub learns what the change is for. A
/// guide too long for GitHub gives only its intent, cut short if need be.
pub(super) fn description(id: &ItemId, guide: Option<&str>) -> String {
    let named = format!("{NAMED} {id}.");
    let Some(guide) = guide.map(str::trim).filter(|guide| !guide.is_empty()) else {
        return named;
    };
    let whole = format!("{named}\n\n{guide}");
    if whole.len() <= MAX_BODY {
        return whole;
    }
    let note = "_The guide is too long for GitHub; read it in mergegate._";
    let intent = Guide::intent(guide);
    let mut end = intent.len().min(MAX_BODY - named.len() - note.len() - 4);
    while !intent.is_char_boundary(end) {
        end -= 1;
    }
    match intent[..end].trim_end() {
        "" => format!("{named}\n\n{note}"),
        intent => format!("{named}\n\n{intent}\n\n{note}"),
    }
}

#[cfg(test)]
#[path = "publish_tests.rs"]
mod tests;
