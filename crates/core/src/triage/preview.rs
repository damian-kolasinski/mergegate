//! What a looser change to the rules would have hidden, shown with it while
//! it waits for the operator.

use super::{RuleId, Rules, Tier, TriagedBy, triage_files};
use crate::diff::DiffStore;
use crate::item::{ItemId, ItemStore};
use crate::project::ProjectName;
use crate::store::StoreError;

/// How many of the project's merged items, newest first, a preview looks at
/// besides the open ones.
pub const MERGED: u32 = 20;

/// Taken once, when the change arrives, so it shows what the operator saw
/// then even as items move on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Preview {
    /// The items looked at.
    pub items: u32,
    /// Files the applied rules review and the change would have trusted.
    pub hidden: Vec<Hidden>,
}

impl Preview {
    /// The items among [`Preview::items`] the change hides a file in.
    pub fn items_hit(&self) -> usize {
        let mut items: Vec<_> = self.hidden.iter().map(|h| &h.item).collect();
        items.dedup();
        items.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hidden {
    pub item: ItemId,
    pub path: String,
    /// The rule that would have trusted it.
    pub rule: RuleId,
}

/// The newest revision of each open item and of the last [`MERGED`] merged
/// ones, triaged by `applied` and by `next`.
///
/// Merged items' diffs are pruned, so a content matcher can't tell on them
/// and keeps their files in review: a `diff_matches` rule may hide more
/// than the preview counts.
pub fn preview<S: ItemStore + DiffStore>(
    store: &S,
    project: &ProjectName,
    applied: &Rules,
    next: &Rules,
) -> Result<Preview, StoreError> {
    let mut items = store.open_items(Some(project))?;
    items.extend(store.merged_items(project, MERGED)?);
    let mut hidden = Vec::new();
    for item in &items {
        let files = store.revision_files(&item.id, item.revision.number)?;
        let (mut before, mut after) = (files.clone(), files);
        triage_files(store, applied, &mut before)?;
        triage_files(store, next, &mut after)?;
        for (before, after) in before.iter().zip(after) {
            if let (Tier::Review, Tier::Trusted, Some(TriagedBy::Rule { id, .. })) =
                (before.triage.tier, after.triage.tier, after.triage.by)
            {
                hidden.push(Hidden {
                    item: item.id.clone(),
                    path: after.file.path,
                    rule: id,
                });
            }
        }
    }
    Ok(Preview {
        items: items.len() as u32,
        hidden,
    })
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
