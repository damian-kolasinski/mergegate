//! Reading an item's spans with its diff: only cached spans, never computed.

use super::{HighlightStore, Highlights, Pack, Span, grammar_for};
use crate::diff::{Hunk, LineKind};
use crate::item::ChangedFile;
use crate::store::StoreError;

/// One file's highlighting, as far as the background pass has got.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileHighlights {
    /// The installed grammar for the file's path.
    pub grammar: Option<String>,
    /// The pack that would highlight the file, when it isn't installed.
    pub missing: Option<String>,
    /// Each side's spans; none while they're being computed.
    pub old: Option<Highlights>,
    pub new: Option<Highlights>,
}

/// `known` are the packs mg can name when none installed covers the file.
pub fn file_highlights(
    store: &impl HighlightStore,
    installed: &[Pack],
    known: &[Pack],
    file: &ChangedFile,
) -> Result<FileHighlights, StoreError> {
    let Some((pack, grammar)) = grammar_for(&file.path, installed) else {
        return Ok(FileHighlights {
            missing: grammar_for(&file.path, known).map(|(p, _)| p.name.clone()),
            ..FileHighlights::default()
        });
    };
    let side = |blob: &Option<String>| match blob {
        Some(blob) => store.highlights(blob, &grammar.name, &pack.version),
        None => Ok(None),
    };
    Ok(FileHighlights {
        old: side(&file.old_blob)?,
        new: side(&file.new_blob)?,
        grammar: Some(grammar.name.clone()),
        missing: None,
    })
}

/// The spans of each of `hunk`'s lines, in order: a removed line's from the
/// old side, any other from the new.
pub fn hunk_spans<'a>(hunk: &Hunk, highlights: &'a FileHighlights) -> Vec<&'a [Span]> {
    let side = |h: &'a Option<Highlights>, n: u32| h.as_ref().map_or(&[][..], |h| h.line(n));
    let (mut old, mut new) = (hunk.old_start, hunk.new_start);
    hunk.lines
        .iter()
        .map(|line| match line.kind {
            LineKind::Removed => {
                old += 1;
                side(&highlights.old, old - 1)
            }
            LineKind::Added => {
                new += 1;
                side(&highlights.new, new - 1)
            }
            LineKind::Context => {
                old += 1;
                new += 1;
                side(&highlights.new, new - 1)
            }
        })
        .collect()
}

/// The cached spans of `blob` under `grammar`, as installed now.
pub fn blob_highlights(
    store: &impl HighlightStore,
    installed: &[Pack],
    grammar: &str,
    blob: &str,
) -> Result<Option<Highlights>, StoreError> {
    let Some(pack) = installed
        .iter()
        .find(|p| p.grammars.iter().any(|g| g.name == grammar))
    else {
        return Ok(None);
    };
    store.highlights(blob, grammar, &pack.version)
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
