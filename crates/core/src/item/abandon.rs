//! Abandoning an item: its agent or the operator gives up on it.

use super::close::close;
use super::stack;
use super::{Item, ItemBackend, ItemError, ItemGit, ItemId, ItemState, ItemStore};
use crate::diff::DiffStore;
use crate::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use crate::github::Github;
use crate::highlight::HighlightStore;

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
///
/// The item stacked on it goes back to its agent with `parent.abandoned`,
/// and its pull request keeps the branch it's based on until it moves: GitHub
/// closes a pull request whose base branch is deleted.
pub fn abandon<S: ItemStore + DiffStore + HighlightStore + FeedbackStore>(
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
    if let ItemBackend::Github { repo, pull } = &item.backend {
        github.close_pull(repo, pull.number)?;
        if store.child(&id)?.is_none() {
            github.delete_branch(repo, &item.branch)?;
        }
        // A closed parent's branch stayed only for this pull request, unless
        // a new item has taken its name since.
        let parent = match &item.parent {
            Some(parent) => store.item(parent)?,
            None => None,
        };
        if let Some(parent) = parent.filter(|parent| !parent.state.is_open())
            && store.open_item(id.project(), &parent.branch)?.is_none()
        {
            let _ = github.delete_branch(repo, &parent.branch);
        }
    }
    let reason = reason
        .map(|r| r.trim().to_owned())
        .filter(|r| !r.is_empty());
    let notice = Notice {
        revision: item.revision.number,
        source,
        kind: NoticeKind::Abandoned { reason },
    };
    let item = close(store, git, &item, ItemState::Abandoned, notice)?;
    stack::abandoned(store, &item)?;
    Ok(item)
}

#[cfg(test)]
#[path = "abandon_tests.rs"]
mod tests;
