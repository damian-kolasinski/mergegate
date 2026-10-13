//! File diffs: computed from two blobs when work is submitted, cached by blob
//! pair, and read back when an item is opened.

mod blob;
mod compute;
mod git;
mod store;

pub use blob::blob;
pub use compute::{CONTEXT, MAX_CHANGED_LINES, compute};
pub use git::DiffGit;
pub use store::DiffStore;

/// One file's change from an old blob to a new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Lines added and removed; none when the change isn't text.
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub content: Content,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// In file order, each with [`CONTEXT`] lines of context.
    Text(Vec<Hunk>),
    /// Not text: a NUL byte in either side's first 8000 bytes, git's own
    /// test, or a side that isn't a blob, as a submodule's commit.
    Binary,
    /// Counted, but over [`MAX_CHANGED_LINES`], so its hunks aren't kept.
    TooLarge,
}

/// Lines as a unified diff's hunk has them. Starts are 1-based, and a side
/// with no lines starts at the line before the hunk, as in git's headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub kind: LineKind,
    /// Without its line break. Bytes that aren't UTF-8 are replaced.
    pub text: String,
    /// False only for a file's last line when the file doesn't end in one.
    pub newline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

impl FileDiff {
    pub fn binary() -> Self {
        Self {
            additions: None,
            deletions: None,
            content: Content::Binary,
        }
    }
}
