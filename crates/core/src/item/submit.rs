//! Registering pushed work as an item or a new revision (F4).

use super::diff_files::diff_files;
use super::{
    Item, ItemError, ItemGit, ItemState, ItemStore, NewItem, NewRevision, valid_branch,
    valid_commit,
};
use crate::agent::{self, AgentStore};
use crate::diff::{DiffGit, DiffStore};
use crate::project::{ProjectStore, find};

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
/// diff is cached before the revision is recorded. A submit at the open
/// item's head still records its worktree and wake.
pub fn submit<S: ProjectStore + ItemStore + DiffStore + AgentStore>(
    store: &mut S,
    git: &(impl ItemGit + DiffGit),
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
    git.fetch_target(&project.name, &project.upstream, &target)?;
    let Some(base) = git.merge_base(&project.name, &head, &target)? else {
        return Err(ItemError::Unrelated { branch, target });
    };
    if base == head {
        return Err(ItemError::NothingToSubmit { branch, target });
    }
    let files = git.changed_files(&project.name, &base, &head)?;
    let files = diff_files(store, git, &project.name, files)?;
    let title = match (title.as_deref().map(str::trim), &existing) {
        (Some(title), _) if !title.is_empty() => title.to_owned(),
        (_, Some(item)) => item.title.clone(),
        _ => git
            .first_subject(&project.name, &base, &head)?
            .filter(|subject| !subject.trim().is_empty())
            .unwrap_or_else(|| branch.clone()),
    };
    let revision = NewRevision { head, base, files };
    let item = match existing {
        Some(item) => store.add_revision(&item.id, &title, ItemState::NeedsReview, revision)?,
        None => store.insert_item(NewItem {
            project: project.name.clone(),
            branch,
            title,
            state: ItemState::NeedsReview,
            backend: project.backend,
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
