//! Opening an item for review: its newest revision with every file's diff,
//! against the base or since an earlier revision.

use std::collections::BTreeMap;

use super::diff_files::compute_diffs;
use super::{ChangedFile, FileStat, Item, ItemError, ItemId, ItemState, ItemStore};
use crate::diff::{Content, DiffGit, DiffStore};
use crate::interdiff::{Change, InterdiffStore};
use crate::judge::JudgeStore;
use crate::review::{self, Conversation, ReviewStore};
use crate::store::StoreError;
use crate::triage::{self, Guide, Hint, Rank, Tier, Triage};
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
    /// The files to review in [`triage::order`], then the trusted ones in
    /// path order. Since an earlier revision, every file the newest changes,
    /// and those the earlier one did that it no longer does.
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
    /// The tier is review for a file the judge promoted, `by` still the
    /// rule that trusted it.
    pub triage: Triage,
    /// Why the guide says to review the file; none for a trusted one.
    pub reason: Option<String>,
    /// How the file changed since the earlier revision; none against the
    /// base.
    pub change: Option<Change>,
    /// The judge's score, 1 to 100; none when it hasn't judged the file.
    pub importance: Option<u8>,
    /// The judge's one line on the file.
    pub judge_reason: Option<String>,
    /// A rule trusted the file and the judge pulled it back into review.
    pub promoted: bool,
    /// The judge's score put the file ahead of the guide's order.
    pub raised: bool,
    pub hint: Option<Hint>,
}

/// `None` when there's no such item. An open item's diffs were all cached at
/// submit, so this only reads. A merged item's are diffed again from staging,
/// where the target's history and its newest revision's pin keep its blobs,
/// and not cached: closing it dropped them on purpose (design §5).
///
/// A closed item shows all changes: its interdiffs' diffs were dropped with
/// the rest.
pub fn item_diff<
    S: ItemStore + DiffStore + ViewedStore + ReviewStore + InterdiffStore + JudgeStore,
>(
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
    let setting = store.judge_setting(id.project())?;
    let mut verdicts = BTreeMap::new();
    if let Some(setting) = &setting {
        let given = store.verdicts(id, newest, &setting.key())?;
        verdicts.extend(
            given
                .unwrap_or_default()
                .into_iter()
                .map(|v| (v.path.clone(), v)),
        );
    }
    let (mut review, mut trusted) = (Vec::new(), Vec::new());
    for ((stat, content), change) in stats.into_iter().zip(contents).zip(changes) {
        let verdict = verdicts.remove(&stat.file.path);
        let mut triage = stat.triage;
        let promoted = triage.tier == Tier::Trusted
            && setting.as_ref().is_some_and(|s| {
                verdict
                    .as_ref()
                    .and_then(|v| v.promote)
                    .is_some_and(|p| p >= s.threshold)
            });
        if promoted {
            triage.tier = Tier::Review;
        }
        let file = DiffedFile {
            // A dropped file isn't one of the newest revision's.
            viewed: change != Some(Change::Dropped) && viewed.contains(&stat.file.path),
            generated: stat.generated,
            reason: (triage.tier == Tier::Review)
                .then(|| parsed.reason(&stat.file.path).map(str::to_owned))
                .flatten(),
            // Nothing in a file unchanged since the earlier revision is new
            // to the operator, so there's nothing to rank it by.
            hint: (change != Some(Change::Unchanged))
                .then(|| {
                    triage::hint(
                        &stat.file.path,
                        stat.generated,
                        stat.additions,
                        stat.deletions,
                        content.as_ref(),
                    )
                })
                .flatten(),
            importance: verdict.as_ref().map(|v| v.importance),
            judge_reason: verdict.and_then(|v| v.reason),
            promoted,
            raised: false,
            triage,
            file: stat.file,
            additions: stat.additions,
            deletions: stat.deletions,
            content,
            change,
        };
        match file.triage.tier {
            Tier::Review => review.push(file),
            Tier::Trusted => trusted.push(file),
        }
    }
    let ranks: Vec<Rank> = review
        .iter()
        .map(|f| Rank {
            path: &f.file.path,
            importance: f.importance,
            hint: f.hint,
        })
        .collect();
    let by = setting.as_ref().map(|s| s.order).unwrap_or_default();
    let placed = triage::order(&ranks, &parsed, by);
    let mut slots: Vec<Option<DiffedFile>> = review.into_iter().map(Some).collect();
    let mut files: Vec<DiffedFile> = placed
        .into_iter()
        .filter_map(|p| {
            let mut file = slots[p.index].take()?;
            file.raised = p.raised;
            Some(file)
        })
        .collect();
    // In path order, as the store lists them: they're there to open, not to
    // work through.
    files.append(&mut trusted);
    let mut conversation = review::read_conversation(store, id, newest)?;
    conversation.placements =
        review::placements(store, git, &item, newest, &conversation.comments)?;
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
