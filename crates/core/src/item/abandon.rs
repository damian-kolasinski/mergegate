//! Abandoning an item: its agent or the operator gives up on it (F14).

use super::close::close;
use super::{Item, ItemError, ItemGit, ItemId, ItemState, ItemStore};
use crate::diff::DiffStore;
use crate::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use crate::github::Github;
use crate::project::{ProjectError, ProjectStore, find};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abandon {
    pub id: ItemId,
    pub source: Source,
    pub reason: Option<String>,
}

/// Any open item can be abandoned, a held merge included. Its agent learns
/// who abandoned it and why from an `item.abandoned` event. A `github`
/// item's pull request is closed and its branch deleted first; the item stays
/// open if GitHub fails, so nothing is left behind there unnoticed.
pub fn abandon<S: ProjectStore + ItemStore + DiffStore + FeedbackStore>(
    store: &mut S,
    git: &impl ItemGit,
    github: &impl Github,
    request: Abandon,
) -> Result<Item, ItemError> {
    let Abandon { id, source, reason } = request;
    let item = store
        .item(&id)?
        .ok_or_else(|| ItemError::NoItem(id.clone()))?;
    if !item.state.is_open() {
        return Err(ItemError::Closed(id));
    }
    if let Some(pull) = &item.pull {
        let project = find(store, item.id.project().as_str())?;
        let repo = project
            .github
            .ok_or(ProjectError::NoGithubSet(project.name))?;
        github.close_pull(&repo, pull.number)?;
        github.delete_branch(&repo, &item.branch)?;
    }
    let reason = reason
        .map(|r| r.trim().to_owned())
        .filter(|r| !r.is_empty());
    let notice = Notice {
        revision: item.revision.number,
        source,
        kind: NoticeKind::Abandoned { reason },
    };
    close(store, git, &item, ItemState::Abandoned, notice)
}

#[cfg(test)]
#[path = "abandon_tests.rs"]
mod tests;
