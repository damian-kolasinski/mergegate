//! Stacks: an item submitted `--on` another is reviewed
//! against the parent revision its branch is built on, lands right after its
//! parent, and hears from the hub whenever its parent moves on.

use super::{IdError, Item, ItemError, ItemGit, ItemId, ItemState, ItemStore, named};
use crate::feedback::{EventKind, FeedbackStore, Notice, NoticeKind, Source};
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
    /// The target has the parent squashed, not its commits, so they'd be
    /// reviewed and land again as the item's own.
    #[error(
        "{parent} merged into {target}, but {branch} still has its commits; move your own onto {target} and submit again"
    )]
    ParentMerged {
        branch: String,
        parent: ItemId,
        target: String,
        upstream: String,
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
    let name = match on {
        Some(On::Target) => return Ok(None),
        Some(On::Item(name)) => name,
        None => {
            let parent = match existing.and_then(|item| item.parent.as_ref()) {
                Some(id) => store.item(id)?,
                None => None,
            };
            return Ok(parent.filter(|parent| parent.state.is_open()));
        }
    };
    if name.project() != project {
        return Err(StackError::OtherProject {
            parent: name,
            project: project.clone(),
        }
        .into());
    }
    let parent = named(store, &name)?.ok_or_else(|| ItemError::NoItem(name.clone()))?;
    // Errors speak in names, as the operator and agents know items by them.
    let id = parent.name();
    if !parent.state.is_open() {
        return Err(StackError::ParentClosed(id).into());
    }
    if let Some(item) = existing {
        if item.id == parent.id {
            return Err(StackError::OnItself(id).into());
        }
        // Its own parents are what it may not be stacked under.
        let mut above = parent.parent.clone();
        while let Some(next) = above {
            if next == item.id {
                return Err(StackError::Cycle {
                    id: item.name(),
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
                id: item.name(),
                target: item.target.clone(),
                parent: id,
                theirs: parent.target.clone(),
            }
            .into());
        }
        if item.backend.kind() != parent.backend.kind() {
            return Err(StackError::OtherBackend {
                id: item.name(),
                parent: id,
            }
            .into());
        }
    }
    if let Some(child) = store.child(&parent.id)?
        && existing.is_none_or(|item| item.id != child.id)
    {
        return Err(StackError::HasChild {
            parent: id,
            child: child.name(),
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
        parent: parent.name(),
        parent_branch: parent.branch.clone(),
    }
    .into())
}

/// Refuses a revision at `head` of `existing`, diffed against `base` on the
/// target, that would show the parent it was stacked on once that merged:
/// one still built on a revision of the parent, where `base` has neither it
/// nor the commit the parent landed as. Merging the target in, or a parent
/// that reached the target whole, leaves nothing to move.
pub(super) fn off_merged_parent<S: ItemStore + FeedbackStore>(
    store: &S,
    git: &impl ItemGit,
    existing: Option<&Item>,
    (branch, head): (&str, &str),
    base: &str,
) -> Result<(), ItemError> {
    let Some(item) = existing else {
        return Ok(());
    };
    let parent = match &item.parent {
        Some(parent) => store.item(parent)?,
        None => None,
    };
    let Some(parent) = parent.filter(|p| p.state == ItemState::Merged) else {
        return Ok(());
    };
    let project = item.id.project();
    // Any of the parent's revisions, not just the one the item was last
    // built on: its agent may have rebased onto a newer one that rewrote it.
    let mut upstream = None;
    for revision in store.revisions(&parent.id)?.iter().rev() {
        if git.contains(project, head, &revision.head)? {
            upstream = Some(revision.head.clone());
            break;
        }
    }
    let Some(upstream) = upstream else {
        return Ok(());
    };
    if git.contains(project, base, &upstream)? {
        return Ok(());
    }
    // Without the landing to tell by, the revision goes as it is.
    let Some(landed) = landed_as(store, &parent.id)? else {
        return Ok(());
    };
    if git.contains(project, base, &landed)? {
        return Ok(());
    }
    Err(StackError::ParentMerged {
        branch: branch.into(),
        parent: parent.name(),
        target: item.target.clone(),
        upstream,
    }
    .into())
}

/// The commit `id` merged as.
fn landed_as(store: &impl FeedbackStore, id: &ItemId) -> Result<Option<String>, StoreError> {
    Ok(store
        .events(id, 0)?
        .into_iter()
        .rev()
        .find_map(|event| match event.kind {
            EventKind::Notice(Notice {
                kind: NoticeKind::Merged { commit },
                ..
            }) => Some(commit),
            _ => None,
        }))
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
    // A parent restacked at the same head has nothing new to rebase onto.
    if child.revision.base == parent.revision.head {
        return Ok(());
    }
    // Not the child's own agent, whose events don't wake it, though the
    // parent's agent sent the revision.
    let notice = Notice {
        revision: child.revision.number,
        source: Source::Operator,
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
) -> Result<Option<Item>, StoreError> {
    let Some(child) = store.child(&parent.id)? else {
        return Ok(None);
    };
    // As in `updated`: whoever abandoned the parent, it wasn't the child's
    // agent, which must wake to move its work.
    let notice = Notice {
        revision: child.revision.number,
        source: Source::Operator,
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

#[cfg(test)]
#[path = "stack_tests.rs"]
mod tests;
