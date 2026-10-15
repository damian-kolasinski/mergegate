//! The background pass: verdicts for open items' newest revisions, asked for
//! after a submit or a judge change rather than when an item is opened.

use std::collections::{BTreeMap, BTreeSet};

use super::request::unified;
use super::{Change, JudgeSetting, JudgeStore, Request, RequestFile, Verdict};
use crate::diff::DiffStore;
use crate::item::{ItemId, ItemStore};
use crate::project::ProjectName;
use crate::store::StoreError;

/// A revision its project's judge hasn't judged.
#[derive(Debug, Clone, PartialEq)]
pub struct Unjudged {
    pub item: ItemId,
    pub revision: u32,
    pub setting: JudgeSetting,
}

/// The newest revision of every open item, only `project`'s when given, that
/// its project's judge has no verdicts on, oldest item first.
pub fn unjudged<S: ItemStore + JudgeStore>(
    store: &S,
    project: Option<&ProjectName>,
) -> Result<Vec<Unjudged>, StoreError> {
    let mut settings = BTreeMap::new();
    let mut unjudged = Vec::new();
    for item in store.open_items(project)? {
        let project = item.id.project();
        if !settings.contains_key(project) {
            settings.insert(project.clone(), store.judge_setting(project)?);
        }
        let Some(setting) = &settings[project] else {
            continue;
        };
        let revision = item.revision.number;
        if store
            .verdicts(&item.id, revision, &setting.key())?
            .is_none()
        {
            unjudged.push(Unjudged {
                item: item.id,
                revision,
                setting: setting.clone(),
            });
        }
    }
    Ok(unjudged)
}

/// What the judge is shown of `revision`; none unless it's still an open
/// item's newest, as the pass judges no other. An open item's diffs were all
/// cached at submit.
pub fn request<S: ItemStore + DiffStore>(
    store: &S,
    id: &ItemId,
    revision: u32,
) -> Result<Option<Request>, StoreError> {
    let Some(item) = store.item(id)? else {
        return Ok(None);
    };
    if !item.state.is_open() || item.revision.number != revision {
        return Ok(None);
    }
    let mut files = Vec::new();
    for stat in store.revision_files(id, revision)? {
        let file = &stat.file;
        let diff = store.diff(file.old_blob.as_deref(), file.new_blob.as_deref())?;
        files.push(RequestFile {
            change: Change::of(file),
            path: stat.file.path,
            old_path: stat.file.old_path,
            additions: stat.additions,
            deletions: stat.deletions,
            tier: stat.triage.tier,
            generated: stat.generated,
            diff: diff.and_then(|d| unified(&d.content)),
        });
    }
    Ok(Some(Request {
        item: id.clone(),
        revision,
        title: item.title,
        guide: store.guide(id, revision)?,
        files,
    }))
}

/// What's kept of a provider's answer: a verdict per file of the request at
/// most, the first given, with values in range; a reason is one trimmed line.
/// A promotion is kept on any file: it only counts on one trusted when the
/// item is read, which may be after a rule change.
pub fn accept(request: &Request, verdicts: Vec<Verdict>) -> Vec<Verdict> {
    let paths: BTreeSet<&str> = request.files.iter().map(|f| f.path.as_str()).collect();
    let mut seen = BTreeSet::new();
    let mut kept: Vec<Verdict> = verdicts
        .into_iter()
        .filter_map(|v| {
            if !paths.contains(v.path.as_str()) || !seen.insert(v.path.clone()) {
                return None;
            }
            let reason = v
                .reason
                .as_deref()
                .and_then(|r| r.lines().map(str::trim).find(|l| !l.is_empty()))
                .map(str::to_owned);
            let promote = v
                .promote
                .filter(|p| p.is_finite())
                .map(|p| p.clamp(0.0, 1.0));
            Some(Verdict {
                importance: v.importance.clamp(1, 100),
                reason,
                promote,
                path: v.path,
            })
        })
        .collect();
    kept.sort_by(|a, b| a.path.cmp(&b.path));
    kept
}

#[cfg(test)]
#[path = "pass_tests.rs"]
mod tests;
