use serde::{Deserialize, Serialize};

use crate::attachment::Demo;
use crate::highlight::Span;
use crate::item::{ChangedSince, Item};
use crate::merge::Blocked;
use crate::review::Conversation;
use crate::rules::Tier;

/// `GET /api/items/{id}?since=`: the item with its newest revision's files.
/// `since` is a revision number or `all`; without it, the files are shown
/// since the last reviewed revision when there's an interdiff from it.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemDiff {
    pub item: Item,
    /// Set while the newest revision's merge is held or conflicts.
    pub blocked: Option<Blocked>,
    /// Null when no revision has attachments.
    pub demo: Option<Demo>,
    /// The revision `files` show the changes since; null for all changes.
    pub since: Option<u32>,
    /// The newest revision the operator submitted a review on.
    pub last_reviewed: Option<u32>,
    /// What the newest revision changes since that review, as the inbox
    /// counts it.
    pub changed_since: Option<ChangedSince>,
    /// The earlier revisions `since` may name, ascending; none once the item
    /// is closed.
    pub interdiffs: Vec<u32>,
    /// The files to review, ranked by the guide, the judge and the
    /// heuristics (design §7.2), then the trusted ones in path order.
    pub files: Vec<File>,
    /// The agent's review guide, Markdown as it sent it.
    pub guide: Option<String>,
    /// The names of the open items the newest revision conflicts with, as on
    /// the inbox: merging this one hands them back to their agents.
    #[serde(default)]
    pub conflicts_with: Vec<String>,
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
    /// Git's mode before and after, null on the side where the file doesn't
    /// exist and on both for a file recorded before the hub kept modes. A
    /// mode can change with no line changing.
    pub old_mode: Option<Mode>,
    pub new_mode: Option<Mode>,
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
    /// Why the guide says to review the file; null for a trusted one.
    pub reason: Option<String>,
    /// How the file changed since `since`; null when showing all changes.
    /// Blobs, counts and content are the interdiff's for `changed`, `added`,
    /// `deleted` and `dropped` files, the revision's own otherwise.
    pub interdiff: Option<Interdiff>,
    /// The judge's score, 1 to 100; null when it hasn't judged the file.
    pub importance: Option<u8>,
    /// The judge's one line on the file.
    pub judge_reason: Option<String>,
    /// A rule trusted the file and the judge pulled it back into review;
    /// `rule` and `why` still name the rule.
    pub promoted: bool,
    /// The judge's score put the file ahead of the guide's order.
    pub raised: bool,
    /// Why the heuristics ranked the file as they did, as in `ranked low:
    /// lockfile`.
    pub hint: Option<String>,
}

/// A file's git mode, as git prints it.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "100644")]
    Regular,
    #[serde(rename = "100755")]
    Executable,
    #[serde(rename = "120000")]
    Symlink,
    /// Its blob is a commit of another repository.
    #[serde(rename = "160000")]
    Submodule,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interdiff {
    /// Changed again since.
    Changed,
    /// Not in the earlier revision: the diff adds the whole file.
    Added,
    /// In the earlier revision, deleted since.
    Deleted,
    /// As it was then, though the base may have moved.
    Unchanged,
    /// Its earlier change conflicts with the target's moves, so this shows
    /// its whole diff.
    Unseparated,
    /// Changed then, not any more: the diff restores the base's version.
    Dropped,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Content {
    /// In file order, each with 3 lines of context.
    Text {
        hunks: Vec<Hunk>,
        /// Unchanged lines after the last hunk, as many on either side, or
        /// every line when there are no hunks. 0 says the last hunk ends
        /// the file; null that the hub doesn't know, for a diff it cached
        /// before it counted them.
        lines_after: Option<u32>,
    },
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
