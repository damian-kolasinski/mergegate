//! Mapping a line range of a file through a diff to its later version.

use crate::diff::{Hunk, LineKind};

/// Where lines `start_line..=line` of a file are in a later version of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapped {
    /// The same lines, unchanged, at these numbers.
    Same { start_line: u32, line: u32 },
    /// Some of them changed: the later lines holding the ones left and those
    /// that replaced the rest.
    Changed { start_line: u32, line: u32 },
    /// Deleted, with nothing in their place.
    Gone,
}

/// Removed and added lines between two unchanged ones: `removed` old lines
/// from `old`, replaced by `added` new ones from `new`. A block that only adds
/// lines puts them before old line `old`.
struct Block {
    old: u32,
    removed: u32,
    new: u32,
    added: u32,
}

/// Maps lines `start_line..=line` of the diff's old side to its new side.
/// `hunks` must hold every change, as a computed diff's do; lines between
/// them are unchanged.
pub fn map(hunks: &[Hunk], start_line: u32, line: u32) -> Mapped {
    let blocks = blocks(hunks);
    let mut first = u32::MAX;
    let mut last = 0;
    let mut include = |from: u32, to: u32| {
        first = first.min(from);
        last = last.max(to);
    };
    let mut changed = false;
    for block in &blocks {
        let touches = if block.removed == 0 {
            // Lines added before the range's first line or after its last
            // leave the range as it was.
            start_line < block.old && block.old <= line
        } else {
            block.old <= line && start_line < block.old + block.removed
        };
        if touches {
            changed = true;
            if block.added > 0 {
                include(block.new, block.new + block.added - 1);
            }
        }
    }
    // Each unchanged line moves by what the blocks before it added and
    // removed; they stay in order, so the first and last are enough.
    let mut delta: i64 = 0;
    let mut next = 0;
    let mut unchanged = (start_line..=line).filter_map(|old| {
        while let Some(block) = blocks.get(next)
            && block.old + block.removed <= old
        {
            delta += i64::from(block.added) - i64::from(block.removed);
            next += 1;
        }
        let removed = blocks
            .get(next)
            .is_some_and(|block| block.removed > 0 && block.old <= old);
        // A mapped line is a line of the new file, so it fits a u32.
        (!removed).then(|| (i64::from(old) + delta) as u32)
    });
    if let Some(from) = unchanged.next() {
        include(from, unchanged.last().unwrap_or(from));
    }
    match (first <= last, changed) {
        (false, _) => Mapped::Gone,
        (true, false) => Mapped::Same {
            start_line: first,
            line: last,
        },
        (true, true) => Mapped::Changed {
            start_line: first,
            line: last,
        },
    }
}

fn blocks(hunks: &[Hunk]) -> Vec<Block> {
    let mut blocks = Vec::new();
    for hunk in hunks {
        // A side without lines starts at the line before the hunk.
        let mut old = hunk.old_start + u32::from(hunk.old_lines == 0);
        let mut new = hunk.new_start + u32::from(hunk.new_lines == 0);
        let mut open: Option<Block> = None;
        for each in &hunk.lines {
            match each.kind {
                LineKind::Context => {
                    blocks.extend(open.take());
                    old += 1;
                    new += 1;
                }
                LineKind::Removed => {
                    open.get_or_insert(Block {
                        old,
                        removed: 0,
                        new,
                        added: 0,
                    })
                    .removed += 1;
                    old += 1;
                }
                LineKind::Added => {
                    open.get_or_insert(Block {
                        old,
                        removed: 0,
                        new,
                        added: 0,
                    })
                    .added += 1;
                    new += 1;
                }
            }
        }
        blocks.extend(open);
    }
    blocks
}

#[cfg(test)]
#[path = "lines_tests.rs"]
mod tests;
