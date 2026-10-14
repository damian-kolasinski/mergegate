//! Reading an item's spans with its diff: only cached spans, never computed.

use super::{HighlightStore, Highlights, Installed, Pack, Span, grammar_for};
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

/// The spans of the lines `hunks` show, as `hunk_spans` reads them. `known`
/// are the packs mg can name when none installed covers the file.
pub fn file_highlights(
    store: &impl HighlightStore,
    installed: &Installed,
    known: &[Pack],
    file: &ChangedFile,
    hunks: &[Hunk],
) -> Result<FileHighlights, StoreError> {
    let Some((_, grammar)) = grammar_for(&file.path, &installed.packs) else {
        return Ok(FileHighlights {
            missing: grammar_for(&file.path, known).map(|(p, _)| p.name.clone()),
            ..FileHighlights::default()
        });
    };
    let (old, new) = shown(hunks);
    let side = |blob: &Option<String>, lines: &[u32]| match blob {
        Some(blob) if !lines.is_empty() => {
            store.highlight_lines(blob, &grammar.name, &installed.version, lines)
        }
        _ => Ok(None),
    };
    Ok(FileHighlights {
        old: side(&file.old_blob, &old)?,
        new: side(&file.new_blob, &new)?,
        grammar: Some(grammar.name.clone()),
        missing: None,
    })
}

/// The line numbers `hunks` show of each side, ascending: removed lines of
/// the old, the others of the new.
fn shown(hunks: &[Hunk]) -> (Vec<u32>, Vec<u32>) {
    let (mut old, mut new) = (Vec::new(), Vec::new());
    for hunk in hunks {
        let (mut o, mut n) = (hunk.old_start, hunk.new_start);
        for line in &hunk.lines {
            match line.kind {
                LineKind::Removed => {
                    old.push(o);
                    o += 1;
                }
                LineKind::Added => {
                    new.push(n);
                    n += 1;
                }
                LineKind::Context => {
                    new.push(n);
                    o += 1;
                    n += 1;
                }
            }
        }
    }
    (old, new)
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
    installed: &Installed,
    grammar: &str,
    blob: &str,
) -> Result<Option<Highlights>, StoreError> {
    let packs = &installed.packs;
    if !packs
        .iter()
        .any(|p| p.grammars.iter().any(|g| g.name == grammar))
    {
        return Ok(None);
    }
    store.highlights(blob, grammar, &installed.version)
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
