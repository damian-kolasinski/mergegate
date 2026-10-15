use std::ops::Range;

use gix_imara_diff::{Algorithm, Diff, InternedInput};

use super::{Content, FileDiff, Hunk, Line, LineKind};

/// Unchanged lines shown around each change. The cache holds hunks with this
/// many, so changing it means clearing the cache.
pub const CONTEXT: u32 = 3;

/// Above this many added and removed lines a file's hunks aren't kept: nobody
/// reviews them line by line, and the view would still have to lay them out.
pub const MAX_CHANGED_LINES: u64 = 20_000;

/// How far into a blob git looks for a NUL byte to call it binary.
const BINARY_PROBE: usize = 8000;

/// Diffs two blobs line by line, with the histogram algorithm git's
/// `diff.algorithm=histogram` uses. An added or deleted file's missing side is
/// empty.
pub fn compute(old: &[u8], new: &[u8]) -> FileDiff {
    if is_binary(old) || is_binary(new) {
        return FileDiff::binary();
    }
    let input = InternedInput::new(old, new);
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    let additions = u64::from(diff.count_additions());
    let deletions = u64::from(diff.count_removals());
    let (content, lines_after) = if additions + deletions > MAX_CHANGED_LINES {
        (Content::TooLarge, None)
    } else {
        let hunks = hunks(&input, &diff, CONTEXT);
        let after = lines_after(&hunks, input.before.len() as u32);
        (Content::Text(hunks), Some(after))
    };
    FileDiff {
        additions: Some(additions),
        deletions: Some(deletions),
        content,
        lines_after,
    }
}

/// Counted on the old side, where the last hunk ends at line `start + lines
/// - 1`, or at `start` when the side is empty.
fn lines_after(hunks: &[Hunk], old_len: u32) -> u32 {
    let end = hunks.last().map_or(0, |hunk| {
        hunk.old_start + hunk.old_lines - u32::from(hunk.old_lines > 0)
    });
    old_len - end
}

fn is_binary(blob: &[u8]) -> bool {
    blob[..blob.len().min(BINARY_PROBE)].contains(&0)
}

/// Groups the changes into hunks the way git does: changes at most
/// `2 * context` unchanged lines apart share a hunk, so contexts never overlap.
fn hunks(input: &InternedInput<&[u8]>, diff: &Diff, context: u32) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    // Where the last change ended on the old side.
    let mut old = 0;
    for change in diff.hunks() {
        let gap = change.before.start - old;
        match hunks.last_mut() {
            Some(hunk) if gap <= 2 * context => {
                push(hunk, input, LineKind::Context, old..change.before.start);
            }
            last => {
                if let Some(hunk) = last {
                    push(hunk, input, LineKind::Context, old..old + context);
                }
                // Unchanged runs are equal on both sides, so the context
                // starts `lead` lines back on either.
                let lead = gap.min(context);
                let start = change.before.start - lead;
                let mut hunk = Hunk {
                    old_start: start,
                    old_lines: 0,
                    new_start: change.after.start - lead,
                    new_lines: 0,
                    lines: Vec::new(),
                };
                push(
                    &mut hunk,
                    input,
                    LineKind::Context,
                    start..change.before.start,
                );
                hunks.push(hunk);
            }
        }
        let hunk = hunks.last_mut().expect("pushed above");
        push(hunk, input, LineKind::Removed, change.before.clone());
        push(hunk, input, LineKind::Added, change.after.clone());
        old = change.before.end;
    }
    let old_len = input.before.len() as u32;
    if let Some(hunk) = hunks.last_mut() {
        push(
            hunk,
            input,
            LineKind::Context,
            old..(old + context).min(old_len),
        );
    }
    for hunk in &mut hunks {
        // 0-based so far; a side with no lines keeps pointing at the line
        // before it.
        hunk.old_start += u32::from(hunk.old_lines > 0);
        hunk.new_start += u32::from(hunk.new_lines > 0);
    }
    hunks
}

/// Appends the lines in `range`, an old-side range for context and removed
/// lines and a new-side one for added lines.
fn push(hunk: &mut Hunk, input: &InternedInput<&[u8]>, kind: LineKind, range: Range<u32>) {
    let tokens = match kind {
        LineKind::Context | LineKind::Removed => &input.before,
        LineKind::Added => &input.after,
    };
    for &token in &tokens[range.start as usize..range.end as usize] {
        let bytes = input.interner[token];
        let text = bytes.strip_suffix(b"\n");
        hunk.lines.push(Line {
            kind,
            text: String::from_utf8_lossy(text.unwrap_or(bytes)).into_owned(),
            newline: text.is_some(),
        });
    }
    let count = range.end - range.start;
    match kind {
        LineKind::Context => {
            hunk.old_lines += count;
            hunk.new_lines += count;
        }
        LineKind::Removed => hunk.old_lines += count,
        LineKind::Added => hunk.new_lines += count,
    }
}

#[cfg(test)]
#[path = "compute_tests.rs"]
mod tests;
