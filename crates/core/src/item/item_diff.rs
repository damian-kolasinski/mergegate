//! Opening an item for review: its newest revision with every file's diff,
//! against the base or since an earlier revision.

use std::collections::BTreeMap;

use super::diff_files::compute_diffs;
use super::{ChangedFile, FileStat, Item, ItemError, ItemId, ItemState, ItemStore};
use crate::diff::{Content, DiffGit, DiffStore};
use crate::interdiff::{Change, InterdiffStore};
use crate::review::{self, Conversation, ReviewStore};
use crate::store::StoreError;
use crate::triage::{Guide, Tier, Triage};
use crate::viewed::ViewedStore;

/// What the newest revision's files are diffed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compare {
    /// Since the revision last reviewed, when that's an earlier one;
    /// against the base otherwise.
    #[default]
    Default,
    /// Against the base.
    All,
    /// Since this earlier revision.
    Since(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDiff {
    pub item: Item,
    /// The earlier revision `files` are diffed from; none when they're
    /// against the base, which is where a comparison without an interdiff
    /// falls back to.
    pub since: Option<u32>,
    /// The newest revision the operator submitted a review on.
    pub last_reviewed: Option<u32>,
    /// The earlier revisions there's an interdiff from, ascending; none once
    /// the item is closed.
    pub interdiffs: Vec<u32>,
    /// The files to review in the guide's order, the rest of them in path
    /// order, then the trusted ones in path order. Since an earlier revision,
    /// every file the newest changes, and those the earlier one did that it
    /// no longer does.
    pub files: Vec<DiffedFile>,
    /// The agent's review guide, as it sent it.
    pub guide: Option<String>,
    pub conversation: Conversation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffedFile {
    pub file: ChangedFile,
    /// Lines added and deleted; none when the change isn't text.
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    /// None once the item is abandoned: its diffs were dropped, and its blobs
    /// may be gone too.
    pub content: Option<Content>,
    /// Marked viewed in the newest revision. A trusted file counts as viewed
    /// without a mark.
    pub viewed: bool,
    pub generated: bool,
    pub triage: Triage,
    /// Why the guide says to review the file; none for a trusted one.
    pub reason: Option<String>,
    /// How the file changed since the earlier revision; none against the
    /// base.
    pub change: Option<Change>,
}

/// `None` when there's no such item. An open item's diffs were all cached at
/// submit, so this only reads. A merged item's are diffed again from staging,
/// where the target's history and its newest revision's pin keep its blobs,
/// and not cached: closing it dropped them on purpose (design §5).
///
/// A closed item shows all changes: its interdiffs' diffs were dropped with
/// the rest.
pub fn item_diff<S: ItemStore + DiffStore + ViewedStore + ReviewStore + InterdiffStore>(
    store: &S,
    git: &impl DiffGit,
    id: &ItemId,
    compare: Compare,
) -> Result<Option<ItemDiff>, ItemError> {
    let Some(item) = store.item(id)? else {
        return Ok(None);
    };
    let newest = item.revision.number;
    let last_reviewed = store.last_reviewed(id)?;
    let interdiffs = if item.state.is_open() {
        store.interdiffs_to(id, newest)?
    } else {
        Vec::new()
    };
    let since = match compare {
        Compare::Default => last_reviewed,
        Compare::All => None,
        Compare::Since(revision) => Some(revision),
    }
    .filter(|revision| interdiffs.contains(revision));
    let (stats, changes): (Vec<FileStat>, Vec<Option<Change>>) = match since {
        Some(from) => {
            // Rules re-triage the revision's own files, so that's where a
            // file's tier is current; a dropped file is reviewed by default.
            let mut own: BTreeMap<String, FileStat> = store
                .revision_files(id, newest)?
                .into_iter()
                .map(|stat| (stat.file.path.clone(), stat))
                .collect();
            store
                .interdiff(id, from, newest)?
                .unwrap_or_default()
                .into_iter()
                .map(|f| {
                    let mut stat = f.stat;
                    if let Some(own) = own.remove(&stat.file.path) {
                        stat.generated = own.generated;
                        stat.triage = own.triage;
                    }
                    (stat, Some(f.change))
                })
                .unzip()
        }
        None => store
            .revision_files(id, newest)?
            .into_iter()
            .map(|stat| (stat, None))
            .unzip(),
    };
    let viewed = store.viewed(id, newest)?;
    let mut contents = Vec::with_capacity(stats.len());
    for stat in &stats {
        let file = &stat.file;
        contents.push(match item.state {
            ItemState::Abandoned => None,
            _ => store
                .diff(file.old_blob.as_deref(), file.new_blob.as_deref())?
                .map(|diff| diff.content),
        });
    }
    let missing: Vec<usize> = (0..stats.len())
        .filter(|&i| item.state != ItemState::Abandoned && contents[i].is_none())
        .collect();
    if item.state == ItemState::Merged {
        let files: Vec<&ChangedFile> = missing.iter().map(|&i| &stats[i].file).collect();
        let diffs = compute_diffs(git, id.project(), &files)?;
        for (&index, diff) in missing.iter().zip(diffs) {
            contents[index] = Some(diff.content);
        }
    } else if let Some(&index) = missing.first() {
        let path = &stats[index].file.path;
        return Err(StoreError::new(format!("{id} has no cached diff for {path}")).into());
    }
    let guide = store.guide(id, item.revision.number)?;
    let parsed = Guide::parse(guide.as_deref().unwrap_or_default());
    let mut files: Vec<DiffedFile> = stats
        .into_iter()
        .zip(contents)
        .zip(changes)
        .map(|((stat, content), change)| DiffedFile {
            // A dropped file isn't one of the newest revision's.
            viewed: change != Some(Change::Dropped) && viewed.contains(&stat.file.path),
            generated: stat.generated,
            reason: (stat.triage.tier == Tier::Review)
                .then(|| parsed.reason(&stat.file.path).map(str::to_owned))
                .flatten(),
            triage: stat.triage,
            file: stat.file,
            additions: stat.additions,
            deletions: stat.deletions,
            content,
            change,
        })
        .collect();
    // Stable, so files the guide doesn't place keep their path order.
    files.sort_by_key(|f| match f.triage.tier {
        Tier::Review => (0, parsed.position(&f.file.path).unwrap_or(usize::MAX)),
        Tier::Trusted => (1, 0),
    });
    let conversation = review::read_conversation(store, id, newest)?;
    Ok(Some(ItemDiff {
        item,
        since,
        last_reviewed,
        interdiffs,
        files,
        guide,
        conversation,
    }))
}

#[cfg(test)]
#[path = "item_diff_tests.rs"]
mod tests;
