//! Stacks (design §5.1, F16): an item submitted `--on` another is reviewed
//! against the parent revision its branch is built on, lands right after its
//! parent, and hears from the hub whenever its parent moves on.

use super::{IdError, Item, ItemBackend, ItemError, ItemGit, ItemId, ItemState, ItemStore};
use crate::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use crate::project::ProjectName;
use crate::store::StoreError;

/// What `mg submit --on` stacks the item on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum On {
    Item(ItemId),
    /// Nothing: the item is reviewed against its target.
    Target,
}

impl On {
    /// `app#41`, `41` for `project`'s item 41, or `none`.
    pub fn parse(text: &str, project: &ProjectName) -> Result<Self, IdError> {
        let text = text.trim();
        if text == "none" {
            return Ok(Self::Target);
        }
        if text.contains('#') {
            return ItemId::parse(text).map(Self::Item);
        }
        ItemId::parse(&format!("{project}#{text}"))
            .map(Self::Item)
            .map_err(|_| IdError::Malformed(text.into()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StackError {
    #[error("{parent} is in another project than {project}")]
    OtherProject {
        parent: ItemId,
        project: ProjectName,
    },
    #[error("{0} is closed; stack on an open item or submit with --on none")]
    ParentClosed(ItemId),
    #[error("{0} can't be stacked on itself")]
    OnItself(ItemId),
    #[error("{parent} is stacked on {id}, so {id} can't be stacked on it")]
    Cycle { id: ItemId, parent: ItemId },
    /// Stacks are linear: a second child would make the first one's base
    /// ambiguous once the parent lands.
    #[error("{child} is already stacked on {parent}; stack on {child} instead")]
    HasChild { parent: ItemId, child: ItemId },
    #[error("{id} merges into {target}, but {parent} merges into {theirs}")]
    OtherTarget {
        id: ItemId,
        target: String,
        parent: ItemId,
        theirs: String,
    },
    #[error("{id} and {parent} merge through different backends")]
    OtherBackend { id: ItemId, parent: ItemId },
    #[error(
        "{branch} has none of {parent}'s revisions; build it on {parent_branch}, or submit with --on none"
    )]
    NotOnParent {
        branch: String,
        parent: ItemId,
        parent_branch: String,
    },
}

/// The open item a submit stacks the item on: the one `on` names, else the
/// one `existing` is stacked on while that's still open. A parent that
/// closed leaves the item's next revision on the target.
pub(super) fn parent_of(
    store: &impl ItemStore,
    project: &ProjectName,
    existing: Option<&Item>,
    on: Option<On>,
) -> Result<Option<Item>, ItemError> {
    let id = match on {
        Some(On::Target) => return Ok(None),
        Some(On::Item(id)) => id,
        None => {
            let parent = match existing.and_then(|item| item.parent.as_ref()) {
                Some(id) => store.item(id)?,
                None => None,
            };
            return Ok(parent.filter(|parent| parent.state.is_open()));
        }
    };
    if id.project() != project {
        return Err(StackError::OtherProject {
            parent: id,
            project: project.clone(),
        }
        .into());
    }
    let parent = store
        .item(&id)?
        .ok_or_else(|| ItemError::NoItem(id.clone()))?;
    if !parent.state.is_open() {
        return Err(StackError::ParentClosed(id).into());
    }
    if let Some(item) = existing {
        if item.id == id {
            return Err(StackError::OnItself(id).into());
        }
        // Its own parents are what it may not be stacked under.
        let mut above = parent.parent.clone();
        while let Some(next) = above {
            if next == item.id {
                return Err(StackError::Cycle {
                    id: item.id.clone(),
                    parent: id,
                }
                .into());
            }
            above = store
                .item(&next)?
                .filter(|i| i.state.is_open())
                .and_then(|i| i.parent);
        }
        if item.target != parent.target {
            return Err(StackError::OtherTarget {
                id: item.id.clone(),
                target: item.target.clone(),
                parent: id,
                theirs: parent.target.clone(),
            }
            .into());
        }
        if item.backend.kind() != parent.backend.kind() {
            return Err(StackError::OtherBackend {
                id: item.id.clone(),
                parent: id,
            }
            .into());
        }
    }
    if let Some(child) = store.child(&id)?
        && existing.is_none_or(|item| item.id != child.id)
    {
        return Err(StackError::HasChild {
            parent: id,
            child: child.id,
        }
        .into());
    }
    Ok(Some(parent))
}

/// What a revision at `head` stacked on `parent` is diffed against: the head
/// of the newest parent revision `head` contains. One built on an older
/// revision is still reviewed by its own diff, and told to rebase.
pub(super) fn base(
    store: &impl ItemStore,
    git: &impl ItemGit,
    parent: &Item,
    branch: &str,
    head: &str,
) -> Result<String, ItemError> {
    let project = parent.id.project();
    for revision in store.revisions(&parent.id)?.iter().rev() {
        if git.contains(project, head, &revision.head)? {
            return Ok(revision.head.clone());
        }
    }
    Err(StackError::NotOnParent {
        branch: branch.into(),
        parent: parent.id.clone(),
        parent_branch: parent.branch.clone(),
    }
    .into())
}

/// The branch the item's pull request merges into: its open parent's, else
/// its target.
pub(crate) fn pull_base<'a>(item_target: &'a str, parent: Option<&'a Item>) -> &'a str {
    match parent {
        Some(parent) if parent.state.is_open() => &parent.branch,
        _ => item_target,
    }
}

/// Tells the item stacked on `parent`, if any, that `parent` has a new
/// revision to rebase onto. It keeps its state: its own diff hasn't changed.
pub(super) fn updated<S: ItemStore + FeedbackStore>(
    store: &mut S,
    parent: &Item,
) -> Result<(), StoreError> {
    let Some(child) = store.child(&parent.id)? else {
        return Ok(());
    };
    let notice = Notice {
        revision: child.revision.number,
        source: Source::Agent,
        kind: NoticeKind::ParentUpdated {
            parent: parent.id.clone(),
            merged: false,
            onto: parent.revision.head.clone(),
            upstream: child.revision.base.clone(),
        },
    };
    store.notify(&child.id, child.state, notice)?;
    Ok(())
}

/// Hands the item stacked on `parent`, if any, back to its agent: what it's
/// built on won't land, so its approval is off and it needs a new base.
/// Returns it, so the caller keeps the branch its pull request is based on.
pub(super) fn abandoned<S: ItemStore + FeedbackStore>(
    store: &mut S,
    parent: &Item,
    source: Source,
) -> Result<Option<Item>, StoreError> {
    let Some(child) = store.child(&parent.id)? else {
        return Ok(None);
    };
    let notice = Notice {
        revision: child.revision.number,
        source,
        kind: NoticeKind::ParentAbandoned {
            parent: parent.id.clone(),
            upstream: child.revision.base.clone(),
        },
    };
    store.notify(&child.id, ItemState::ChangesRequested, notice)?;
    Ok(Some(child))
}

/// An item's place in its stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stack {
    /// Bottom first, the item among them: its parents down to the first
    /// closed one, then its children. Only the item, when it isn't stacked.
    pub items: Vec<Item>,
}

/// The stack `item` is in.
pub fn stack(store: &impl ItemStore, item: &Item) -> Result<Stack, StoreError> {
    let mut items = vec![item.clone()];
    let mut below = item.parent.clone();
    while let Some(id) = below {
        // A cycle can't be stored, but a read shouldn't loop if one were.
        if items.iter().any(|i| i.id == id) {
            break;
        }
        let Some(parent) = store.item(&id)? else {
            break;
        };
        below = parent.parent.clone().filter(|_| parent.state.is_open());
        items.insert(0, parent);
    }
    if item.state.is_open() {
        let mut above = item.id.clone();
        while let Some(child) = store.child(&above)? {
            if items.iter().any(|i| i.id == child.id) {
                break;
            }
            above = child.id.clone();
            items.push(child);
        }
    }
    Ok(Stack { items })
}

/// Whether `item` is built on something its parent no longer is: a newer
/// parent revision, or a parent that closed. Its agent has heard, and its
/// next revision rebases.
pub fn behind(store: &impl ItemStore, item: &Item) -> Result<bool, StoreError> {
    let Some(id) = &item.parent else {
        return Ok(false);
    };
    Ok(match store.item(id)? {
        Some(parent) => !parent.state.is_open() || parent.revision.head != item.revision.base,
        None => false,
    })
}

impl Item {
    /// Whether its pull request, if it has one, is based on another item's
    /// branch: the parent's while that's open, and while it's abandoned
    /// until the item moves.
    pub fn stacked_pull(&self) -> bool {
        self.parent.is_some() && matches!(self.backend, ItemBackend::Github { .. })
    }
}

#[cfg(test)]
#[path = "stack_tests.rs"]
mod tests;
