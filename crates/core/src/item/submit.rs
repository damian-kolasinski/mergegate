//! Registering pushed work as an item or a new revision (F4).

use super::diff_files::diff_files;
use super::publish::{Revision, publish};
use super::{
    Item, ItemBackend, ItemError, ItemGit, ItemState, ItemStore, NewItem, NewRevision,
    valid_branch, valid_commit,
};
use crate::agent::{self, AgentStore};
use crate::diff::{DiffGit, DiffStore};
use crate::git::Upstream;
use crate::github::Github;
use crate::project::{Backend, ProjectError, ProjectStore, find};
use crate::triage::{RulesStore, TriageGit, triage_files};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submit {
    pub project: String,
    pub branch: String,
    pub head: String,
    pub title: Option<String>,
    /// The submitting worktree's path, which `mg resume` names.
    pub worktree: Option<String>,
    /// The command that wakes the item's agent; empty removes it. Kept when
    /// none is sent.
    pub wake: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submitted {
    pub item: Item,
    pub new_revision: bool,
}

/// The branch must already be pushed to staging at `request.head`. The
/// target is fetched fresh first, so the merge-base is current. Every file's
/// diff is cached and the file triaged by the applied rules before the
/// revision is recorded. A submit at the open item's head still records its
/// worktree and wake.
///
/// A `github` item is published before it's recorded: its branch pushed and
/// its pull request opened or retitled. If GitHub fails, the submit fails
/// and nothing is recorded.
pub fn submit<S: ProjectStore + ItemStore + DiffStore + AgentStore + RulesStore>(
    store: &mut S,
    git: &(impl ItemGit + DiffGit + TriageGit),
    github: &impl Github,
    request: Submit,
) -> Result<Submitted, ItemError> {
    let project = find(store, &request.project)?;
    let Submit {
        branch,
        head,
        title,
        worktree,
        wake,
        ..
    } = request;
    if !valid_branch(&branch) {
        return Err(ItemError::InvalidBranch(branch));
    }
    if !valid_commit(&head) {
        return Err(ItemError::InvalidCommit(head));
    }
    let existing = store.open_item(&project.name, &branch)?;
    let target = existing
        .as_ref()
        .map_or(&project.target, |item| &item.target)
        .clone();
    if branch == target || branch == project.target {
        return Err(ItemError::TargetBranch { branch });
    }
    if let Some(item) = existing.as_ref().filter(|i| i.revision.head == head) {
        // Pinning again repairs a submit that failed after recording.
        git.pin(&project.name, &item.pin_ref(), &head)?;
        agent::record(store, &item.id, worktree, wake)?;
        return Ok(Submitted {
            item: item.clone(),
            new_revision: false,
        });
    }
    match git.branch_head(&project.name, &branch)? {
        None => return Err(ItemError::NoBranch(branch)),
        Some(found) if found != head => {
            return Err(ItemError::NotPushed {
                branch,
                head,
                found,
            });
        }
        Some(_) => {}
    }
    // An open item stays where it was opened, whatever the project says now.
    let github_repo = match existing.as_ref().map(|item| &item.backend) {
        Some(ItemBackend::Github { repo, .. }) => Some(repo.clone()),
        Some(ItemBackend::Local) => None,
        None => match project.backend {
            Backend::Github => Some(
                project
                    .github
                    .clone()
                    .ok_or_else(|| ProjectError::NoGithubSet(project.name.clone()))?,
            ),
            Backend::Local => None,
        },
    };
    let upstream = match &github_repo {
        Some(repo) => Upstream::Remote(github.remote(repo)?),
        None => Upstream::Repo(project.upstream.clone()),
    };
    git.fetch_target(&project.name, &upstream, &target)?;
    let Some(base) = git.merge_base(&project.name, &head, &target)? else {
        return Err(ItemError::Unrelated { branch, target });
    };
    if base == head {
        return Err(ItemError::NothingToSubmit { branch, target });
    }
    let files = git.changed_files(&project.name, &base, &head)?;
    let mut files = diff_files(store, git, &project.name, files)?;
    // A rename counts as generated only when both its paths are, as code
    // moved into a generated directory isn't.
    let paths: Vec<&str> = files
        .iter()
        .flat_map(|f| std::iter::once(&f.file.path).chain(&f.file.old_path))
        .map(String::as_str)
        .collect();
    // The base's attributes, so a revision can't mark its own files
    // generated; the guard keeps a changed `.gitattributes` reviewed too.
    let mut generated = git.generated(&project.name, &base, &paths)?.into_iter();
    for file in &mut files {
        let renamed = file.file.old_path.is_some();
        let new = generated.next().unwrap_or(false);
        let old = !renamed || generated.next().unwrap_or(false);
        file.generated = new && old;
    }
    let rules = store.applied_rules(&project.name)?;
    triage_files(store, &rules, &mut files)?;
    let title = match (title.as_deref().map(str::trim), &existing) {
        (Some(title), _) if !title.is_empty() => title.to_owned(),
        (_, Some(item)) => item.title.clone(),
        _ => git
            .first_subject(&project.name, &base, &head)?
            .filter(|subject| !subject.trim().is_empty())
            .unwrap_or_else(|| branch.clone()),
    };
    let backend = match (&upstream, github_repo) {
        (Upstream::Remote(remote), Some(repo)) => {
            let pull = publish(
                git,
                github,
                &project.name,
                (&repo, remote),
                existing.as_ref(),
                Revision {
                    branch: &branch,
                    head: &head,
                    target: &target,
                    title: &title,
                },
            )?;
            ItemBackend::Github { repo, pull }
        }
        _ => ItemBackend::Local,
    };
    let revision = NewRevision { head, base, files };
    let item = match existing {
        Some(item) => store.add_revision(&item.id, &title, ItemState::NeedsReview, revision)?,
        None => store.insert_item(NewItem {
            project: project.name.clone(),
            branch,
            title,
            state: ItemState::NeedsReview,
            backend,
            target,
            revision,
        })?,
    };
    git.pin(&project.name, &item.pin_ref(), &item.revision.head)?;
    agent::record(store, &item.id, worktree, wake)?;
    Ok(Submitted {
        item,
        new_revision: true,
    })
}

#[cfg(test)]
#[path = "submit_tests.rs"]
mod tests;
