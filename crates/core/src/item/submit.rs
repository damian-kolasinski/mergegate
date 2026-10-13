//! Registering pushed work as an item or a new revision (F4).

use super::{
    Item, ItemError, ItemGit, ItemState, ItemStore, NewItem, NewRevision, valid_branch,
    valid_commit,
};
use crate::project::{ProjectStore, find};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submit {
    pub project: String,
    pub branch: String,
    /// The commit the worker pushed `branch` to.
    pub head: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submitted {
    pub item: Item,
    /// False when the branch was already submitted at this head.
    pub new_revision: bool,
}

/// Opens an item for `request.branch`, or adds a revision to the branch's
/// open item. The branch must already be pushed to staging at
/// `request.head`. Its diff is taken against the merge-base with the item's
/// target, fetched fresh from the upstream.
pub fn submit<S: ProjectStore + ItemStore>(
    store: &mut S,
    git: &impl ItemGit,
    request: Submit,
) -> Result<Submitted, ItemError> {
    let project = find(store, &request.project)?;
    let Submit {
        branch,
        head,
        title,
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
    Ok(Submitted {
        item,
        new_revision: true,
    })
}

#[cfg(test)]
#[path = "submit_tests.rs"]
mod tests;
