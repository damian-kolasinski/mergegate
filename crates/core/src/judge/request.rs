use std::fmt::Write;

use crate::diff::{Content, LineKind};
use crate::item::{ChangedFile, ItemId};
use crate::triage::Tier;

/// The whole change, as a judge sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub item: ItemId,
    pub revision: u32,
    pub title: String,
    /// The agent's review guide, as it sent it.
    pub guide: Option<String>,
    /// In path order.
    pub files: Vec<RequestFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestFile {
    pub path: String,
    pub old_path: Option<String>,
    pub change: Change,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub tier: Tier,
    pub generated: bool,
    /// Unified hunks; none when the change isn't text or is too large to
    /// keep.
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Added,
    Deleted,
    Renamed,
    Modified,
}

impl Change {
    pub fn of(file: &ChangedFile) -> Self {
        match (&file.old_blob, &file.new_blob, &file.old_path) {
            (None, _, _) => Self::Added,
            (_, None, _) => Self::Deleted,
            (_, _, Some(_)) => Self::Renamed,
            _ => Self::Modified,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Deleted => "deleted",
            Self::Renamed => "renamed",
            Self::Modified => "modified",
        }
    }
}

/// `content`'s hunks as `git diff` prints them, without the file header.
pub(super) fn unified(content: &Content) -> Option<String> {
    let Content::Text(hunks) = content else {
        return None;
    };
    let mut out = String::new();
    for hunk in hunks {
        let _ = writeln!(
            out,
            "@@ -{},{} +{},{} @@",
            hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
        );
        for line in &hunk.lines {
            out.push(match line.kind {
                LineKind::Context => ' ',
                LineKind::Added => '+',
                LineKind::Removed => '-',
            });
            out.push_str(&line.text);
            out.push('\n');
        }
    }
    Some(out)
}
