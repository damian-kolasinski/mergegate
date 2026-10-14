//! Opening an item for review: its newest revision with every file's diff.

use super::diff_files::compute_diffs;
use super::{ChangedFile, Item, ItemError, ItemId, ItemState, ItemStore};
use crate::diff::{Content, DiffGit, DiffStore};
use crate::review::{self, Conversation, ReviewStore};
use crate::store::StoreError;
use crate::triage::{Guide, Tier, Triage};
use crate::viewed::ViewedStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDiff {
    pub item: Item,
    /// The files to review in the guide's order, the rest of them in path
    /// order, then the trusted ones in path order.
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
    /// Marked viewed in this revision. A trusted file counts as viewed
    /// without a mark.
    pub viewed: bool,
    pub generated: bool,
    pub triage: Triage,
    /// Why the guide says to review the file; none for a trusted one.
    pub reason: Option<String>,
}

/// `None` when there's no such item. An open item's diffs were all cached at
/// submit, so this only reads. A merged item's are diffed again from staging,
/// where the target's history and its newest revision's pin keep its blobs,
/// and not cached: closing it dropped them on purpose (design §5).
pub fn item_diff<S: ItemStore + DiffStore + ViewedStore + ReviewStore>(
    store: &S,
    git: &impl DiffGit,
    id: &ItemId,
) -> Result<Option<ItemDiff>, ItemError> {
    let Some(item) = store.item(id)? else {
        return Ok(None);
    };
    let viewed = store.viewed(id, item.revision.number)?;
    let stats = store.revision_files(id, item.revision.number)?;
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
        .map(|(stat, content)| DiffedFile {
            viewed: viewed.contains(&stat.file.path),
            generated: stat.generated,
            reason: (stat.triage.tier == Tier::Review)
                .then(|| parsed.reason(&stat.file.path).map(str::to_owned))
                .flatten(),
            triage: stat.triage,
            file: stat.file,
            additions: stat.additions,
            deletions: stat.deletions,
            content,
        })
        .collect();
    // Stable, so files the guide doesn't place keep their path order.
    files.sort_by_key(|f| match f.triage.tier {
        Tier::Review => (0, parsed.position(&f.file.path).unwrap_or(usize::MAX)),
        Tier::Trusted => (1, 0),
    });
    let conversation = review::read_conversation(store, id, item.revision.number)?;
    Ok(Some(ItemDiff {
        item,
        files,
        guide,
        conversation,
    }))
}

#[cfg(test)]
#[path = "item_diff_tests.rs"]
mod tests;
