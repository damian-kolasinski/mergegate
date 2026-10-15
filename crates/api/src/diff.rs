use serde::{Deserialize, Serialize};

use crate::attachment::Demo;
use crate::highlight::Span;
use crate::item::Item;
use crate::merge::Blocked;
use crate::review::Conversation;
use crate::rules::Tier;

/// `GET /api/items/{id}`: the item with its newest revision's files.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemDiff {
    pub item: Item,
    /// Set while the newest revision's merge is held or conflicts.
    pub blocked: Option<Blocked>,
    /// Null when no revision has attachments.
    pub demo: Option<Demo>,
    /// In path order.
    pub files: Vec<File>,
    #[serde(flatten)]
    pub conversation: Conversation,
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
    /// Marked viewed in this revision. A trusted file counts as viewed
    /// without a mark.
    pub viewed: bool,
    /// The installed grammar that highlights the file, by its path; `GET
    /// /api/highlights/{grammar}/{blob}` reads a side's spans.
    pub grammar: Option<String>,
    /// The language pack that would highlight the file, when it isn't
    /// installed.
    pub missing_pack: Option<String>,
    /// `linguist-generated` as the revision's base sets it.
    pub generated: bool,
    pub tier: Tier,
    /// The rule that set the tier, as in `R3`; null when nothing matched or
    /// a guard kept the file in review.
    pub rule: Option<String>,
    /// Why the file is in its tier: the rule's matchers and reason when it
    /// matched, or the guard's. Null when nothing matched.
    pub why: Option<String>,
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
    /// The item was abandoned, which dropped its diffs; only counted.
    Gone,
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
    /// Each of `lines`' spans, in column order and without the prefix: a
    /// removed line's from the old side, any other's from the new. Empty
    /// while the hub is still computing them, and for `\ No newline`
    /// lines.
    pub spans: Vec<Vec<Span>>,
}
