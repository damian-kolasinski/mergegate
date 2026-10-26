//! Whether an open item's newest revision merges cleanly into its target,
//! for the operator to see before approving it: what the watch last found,
//! and what its agent was told.

use std::collections::{BTreeMap, BTreeSet};

use super::Seen;
use crate::feedback::{EventKind, FeedbackStore, Notice, NoticeKind};
use crate::item::{Item, ItemId, ItemState};
use crate::project::ProjectName;
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mergeable {
    Clean,
    Conflict {
        target_commit: String,
        paths: Vec<String>,
    },
    /// Not tried at the target as last fetched, the try failed, or it can't
    /// be told before the item lands.
    Unknown,
}

/// The revisions each project's last pass found merging cleanly, as item
/// number and head. Kept in memory: a restarted hub's first pass tries every
/// item again.
#[derive(Debug, Default)]
pub struct Clean(BTreeMap<ProjectName, BTreeSet<(u64, String)>>);

impl Clean {
    /// Replaces what was known of `project`'s items with what its last pass
    /// saw, an empty `seen` forgetting them. Returns the items that started
    /// or stopped merging cleanly, for the clients to refresh.
    pub fn record(&mut self, project: &ProjectName, seen: &Seen) -> Vec<ItemId> {
        let clean: BTreeSet<(u64, String)> = seen
            .clean()
            .map(|(number, head)| (number, head.to_owned()))
            .collect();
        let was = self.0.insert(project.clone(), clean).unwrap_or_default();
        let now = &self.0[project];
        let changed: BTreeSet<u64> = was
            .symmetric_difference(now)
            .map(|(number, _)| *number)
            .collect();
        changed
            .into_iter()
            .map(|number| ItemId::new(project.clone(), number))
            .collect()
    }

    /// Forgets the projects not in `projects`, as once one is removed.
    pub fn retain(&mut self, projects: &[ProjectName]) {
        self.0.retain(|name, _| projects.contains(name));
    }

    fn has(&self, item: &Item) -> bool {
        self.0
            .get(item.id.project())
            .is_some_and(|clean| clean.contains(&(item.id.number(), item.revision.head.clone())))
    }
}

/// A conflict its agent was told of on the newest revision stands, as the
/// watch doesn't try that revision again; else the watch's last pass says.
/// Unknown once the item is approved, as the watch leaves it to its merge,
/// whose outcome the item's `Blocked` says, and once it's closed.
///
/// An item stacked on another is unknown too: its branch carries its
/// parent's commits and lands only after its parent, when its own diff is
/// tried onto a target that by then has the parent. A trial now would report
/// the parent's conflicts as its own.
pub fn mergeable(
    store: &impl FeedbackStore,
    clean: &Clean,
    item: &Item,
) -> Result<Mergeable, StoreError> {
    let reviewing = matches!(
        item.state,
        ItemState::NeedsReview | ItemState::ChangesRequested
    );
    if !reviewing || item.parent.is_some() {
        return Ok(Mergeable::Unknown);
    }
    if let Some(conflict) = told_conflict(store, item)? {
        return Ok(conflict);
    }
    Ok(match clean.has(item) {
        true => Mergeable::Clean,
        false => Mergeable::Unknown,
    })
}

/// The last conflict on the newest revision its agent heard of, from the
/// watch or a merge.
pub(super) fn told_conflict(
    store: &impl FeedbackStore,
    item: &Item,
) -> Result<Option<Mergeable>, StoreError> {
    Ok(store
        .events(&item.id, 0)?
        .into_iter()
        .filter_map(|event| match event.kind {
            EventKind::Notice(Notice {
                revision,
                kind:
                    NoticeKind::MergeConflict {
                        target_commit,
                        paths,
                    },
                ..
            }) if revision == item.revision.number => Some(Mergeable::Conflict {
                target_commit,
                paths,
            }),
            _ => None,
        })
        .next_back())
}

#[cfg(test)]
#[path = "mergeable_tests.rs"]
mod tests;
