//! Open items test-merged into each other: what the inbox shows as
//! "conflicts with". It reaches no agent, since only landing one of them
//! makes it a conflict, and the target watch tells the other's agent then.
//!
//! As with the watch, the merges run without the store, between a read and
//! a save under it.

use std::collections::BTreeMap;

use super::{ConflictError, ConflictGit, ConflictStore, Pair};
use crate::item::{Item, ItemId, ItemStore};
use crate::project::ProjectName;

/// A project's open items and the pairs last tried among them.
#[derive(Debug)]
pub struct Pairing {
    project: ProjectName,
    open: Vec<Item>,
    cached: Vec<Pair>,
}

impl Pairing {
    pub fn read<S: ItemStore + ConflictStore>(
        store: &S,
        project: &ProjectName,
    ) -> Result<Self, ConflictError> {
        let mut open = store.open_items(Some(project))?;
        open.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self {
            project: project.clone(),
            open,
            cached: store.pairs(Some(project))?,
        })
    }

    /// Every two open items with one target, Merging included, as one may
    /// land any moment, except two of one stack, which land in turn. A pair
    /// is merged again only once a head moves.
    pub fn try_all(&self, git: &impl ConflictGit) -> Result<Vec<Pair>, ConflictError> {
        let mut tried = Vec::new();
        for (i, item) in self.open.iter().enumerate() {
            let bottom = bottom(&self.open, item);
            for other in self.open[i + 1..]
                .iter()
                .filter(|o| o.target == item.target && self::bottom(&self.open, o) != bottom)
            {
                let (item_head, other_head) = (&item.revision.head, &other.revision.head);
                let known = self.cached.iter().find(|pair| {
                    pair.item == item.id
                        && pair.other == other.id
                        && pair.item_head == *item_head
                        && pair.other_head == *other_head
                });
                let conflicts = match known {
                    Some(pair) => pair.conflicts,
                    None => !git
                        .conflicts(&self.project, item_head, other_head)?
                        .is_empty(),
                };
                tried.push(Pair {
                    item: item.id.clone(),
                    item_head: item_head.clone(),
                    other: other.id.clone(),
                    other_head: other_head.clone(),
                    conflicts,
                });
            }
        }
        Ok(tried)
    }
}

/// Replaces the project's pairs with `tried`, and returns the items of every
/// conflicting pair found or dropped, whose "conflicts with" an open page may
/// still show as it was. Pairs at heads that moved since they were tried show
/// nothing, and the pass the move asked for tries them; a page fetched before
/// the move still names the other item, so a dropped stale pair counts too.
pub fn save_pairs<S: ConflictStore>(
    store: &mut S,
    project: &ProjectName,
    tried: &[Pair],
) -> Result<Vec<ItemId>, ConflictError> {
    let cached = store.pairs(Some(project))?;
    if tried == cached {
        return Ok(Vec::new());
    }
    store.set_pairs(project, tried)?;
    let only_in = |pairs: &[Pair], other: &[Pair]| -> Vec<ItemId> {
        pairs
            .iter()
            .filter(|pair| pair.conflicts && !other.contains(pair))
            .flat_map(|pair| [pair.item.clone(), pair.other.clone()])
            .collect()
    };
    let mut changed = only_in(&cached, tried);
    changed.extend(only_in(tried, &cached));
    changed.sort();
    changed.dedup();
    Ok(changed)
}

/// The open items each of `open` conflicts with, by `pairs` tried at their
/// heads now. An item with none isn't listed.
///
/// A stacked item's branch carries its open parents' commits, so it
/// conflicts with whatever they do: that conflict is listed on the parent
/// only.
pub fn conflicting(open: &[Item], pairs: &[Pair]) -> BTreeMap<ItemId, Vec<ItemId>> {
    let head = |id: &ItemId| {
        open.iter()
            .find(|item| item.id == *id)
            .map(|item| &item.revision.head)
    };
    let mut conflicts: BTreeMap<ItemId, Vec<ItemId>> = BTreeMap::new();
    for pair in pairs.iter().filter(|pair| pair.conflicts) {
        if head(&pair.item) == Some(&pair.item_head) && head(&pair.other) == Some(&pair.other_head)
        {
            for (one, two) in [(&pair.item, &pair.other), (&pair.other, &pair.item)] {
                conflicts.entry(one.clone()).or_default().push(two.clone());
            }
        }
    }
    let inherited = |id: &ItemId, other: &ItemId| {
        parents(open, id).any(|parent| {
            conflicts
                .get(&parent.id)
                .is_some_and(|with| with.contains(other))
        })
    };
    let mut listed: BTreeMap<ItemId, Vec<ItemId>> = BTreeMap::new();
    for (id, with) in &conflicts {
        let own: Vec<_> = with
            .iter()
            .filter(|other| !inherited(id, other) && !inherited(other, id))
            .cloned()
            .collect();
        if !own.is_empty() {
            listed.insert(id.clone(), own);
        }
    }
    for with in listed.values_mut() {
        with.sort();
    }
    listed
}

/// The open items `id` is stacked on, nearest first.
fn parents<'a>(open: &'a [Item], id: &ItemId) -> impl Iterator<Item = &'a Item> {
    let find = move |id: &ItemId| open.iter().find(|item| item.id == *id);
    let first = find(id)
        .and_then(|item| item.parent.as_ref())
        .and_then(find);
    // Bounded, so a cycle the store can't hold wouldn't loop.
    std::iter::successors(first, move |item| item.parent.as_ref().and_then(find)).take(open.len())
}

/// The bottom of the stack `item` is in, of the open items: itself when it
/// isn't stacked on one.
fn bottom<'a>(open: &'a [Item], item: &'a Item) -> &'a ItemId {
    parents(open, &item.id)
        .last()
        .map_or(&item.id, |bottom| &bottom.id)
}

#[cfg(test)]
#[path = "pairs_tests.rs"]
mod tests;
