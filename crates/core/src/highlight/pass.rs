//! The background pass: spans for the blobs open items show, computed after
//! a submit or a pack install rather than when an item is opened.

use std::collections::BTreeSet;

use super::{HighlightStore, Highlighter, Highlights, Installed, grammar_for};
use crate::diff::DiffGit;
use crate::git::GitError;
use crate::interdiff::InterdiffStore;
use crate::item::ItemStore;
use crate::project::ProjectName;
use crate::store::StoreError;

/// Larger blobs aren't highlighted: a parse is linear in its input, but the
/// spans of a huge generated file aren't worth their time or rows.
pub const MAX_BYTES: usize = 1 << 20;

/// A blob an open item shows that its file's installed grammar hasn't
/// highlighted yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub project: ProjectName,
    pub blob: String,
    pub grammar: String,
    pub version: String,
}

/// Drops the spans computed with other packs installed, then lists the blobs of
/// every open item's newest revision, and of the interdiffs to it, that lack
/// spans. Both sides of a file take the grammar of its new path. Files whose
/// change isn't text are skipped: they're never shown as lines.
pub fn pending<S: ItemStore + InterdiffStore + HighlightStore>(
    store: &mut S,
    installed: &Installed,
) -> Result<Vec<Pending>, StoreError> {
    let version = &installed.version;
    store.retain_highlights(version)?;

    let mut seen = BTreeSet::new();
    let mut pending = Vec::new();
    for item in store.open_items(None)? {
        let newest = item.revision.number;
        let mut stats = store.revision_files(&item.id, newest)?;
        // An interdiff's old side is an earlier revision replayed onto the
        // newest's base, and a dropped file's new side is that base's: blobs
        // no revision's own files have.
        for from in store.interdiffs_to(&item.id, newest)? {
            let files = store.interdiff(&item.id, from, newest)?;
            stats.extend(files.into_iter().flatten().map(|f| f.stat));
        }
        for stat in stats {
            if stat.additions.is_none() {
                continue;
            }
            let Some((_, grammar)) = grammar_for(&stat.file.path, &installed.packs) else {
                continue;
            };
            for blob in [&stat.file.old_blob, &stat.file.new_blob]
                .into_iter()
                .flatten()
            {
                if !seen.insert((blob.clone(), grammar.name.clone()))
                    || store.has_highlights(blob, &grammar.name, version)?
                {
                    continue;
                }
                pending.push(Pending {
                    project: item.id.project().clone(),
                    blob: blob.clone(),
                    grammar: grammar.name.clone(),
                    version: version.clone(),
                });
            }
        }
    }
    Ok(pending)
}

/// `pending`'s spans. A blob that's missing, too large or that its grammar
/// fails on gets none, so the pass doesn't try it again.
pub fn compute(
    git: &impl DiffGit,
    highlighter: &impl Highlighter,
    pending: &Pending,
) -> Result<Highlights, GitError> {
    let blob = git
        .blobs(&pending.project, &[&pending.blob])?
        .pop()
        .flatten();
    let Some(bytes) = blob.filter(|b| b.len() <= MAX_BYTES) else {
        return Ok(Highlights::default());
    };
    // The diff's lines replace bytes that aren't UTF-8 the same way, so the
    // spans' columns line up with what the UI shows.
    let text = String::from_utf8_lossy(&bytes);
    Ok(highlighter
        .highlight(&pending.grammar, &text)
        .unwrap_or_default())
}

#[cfg(test)]
#[path = "pass_tests.rs"]
mod tests;
