use serde::{Deserialize, Serialize};

use crate::item::Item;

/// `GET /api/items/{id}`: the item with its newest revision's files.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemDiff {
    pub item: Item,
    /// In path order.
    pub files: Vec<File>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    pub path: String,
    /// Set when the file was renamed.
    pub old_path: Option<String>,
    /// Object IDs before and after, null on the side where the file doesn't
    /// exist; `GET /api/projects/{name}/blobs/{id}` reads them. A submodule's
    /// are commits.
    pub old_blob: Option<String>,
    pub new_blob: Option<String>,
    /// Null when the change isn't text.
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub content: Content,
    /// Marked viewed in this revision.
    pub viewed: bool,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Content {
    /// In file order, each with 3 lines of context.
    Text { hunks: Vec<Hunk> },
    /// Either side isn't text, or isn't a blob.
    Binary,
    /// Too many changed lines to show; only counted.
    TooLarge,
}

/// A unified diff's hunk. Starts are 1-based, and a side with no lines starts
/// at the line before the hunk, as in git's headers.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    /// As a unified diff has them: prefixed with ' ', '+' or '-', without the
    /// line break, and followed by `\ No newline at end of file` when the
    /// line has none.
    pub lines: Vec<String>,
}
