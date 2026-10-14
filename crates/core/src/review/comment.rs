use std::fmt;

use crate::item::ItemId;

/// How many of the commented lines a comment quotes, the last ones of its
/// range: enough for the agent to know what it's about without the file.
pub const QUOTE_LINES: usize = 5;

/// Hub-wide and never reused, so an agent can name a comment on its own, as
/// in `mg reply c_17`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommentId(pub u64);

impl CommentId {
    pub fn parse(text: &str) -> Option<Self> {
        let number: u64 = text.strip_prefix("c_")?.parse().ok()?;
        // `parse` takes a sign and leading zeros, which would make two
        // spellings of one ID.
        (number > 0 && text == format!("c_{number}")).then_some(Self(number))
    }
}

impl fmt::Display for CommentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "c_{}", self.0)
    }
}

/// A comment on lines `start_line..=line` of one side of a file in one
/// revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: CommentId,
    pub item: ItemId,
    pub revision: u32,
    pub path: String,
    pub side: Side,
    pub start_line: u32,
    pub line: u32,
    pub body: String,
    /// The commented lines, at most [`QUOTE_LINES`] of them.
    pub quote: String,
    /// The review that sent it; none while it's a draft.
    pub review: Option<u64>,
    pub created_at: String,
}

/// The side of the diff a comment's lines are on: the file before the change
/// or after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Old,
    New,
}

impl Side {
    pub const ALL: [Self; 2] = [Self::Old, Self::New];

    /// The name the store spells the side with.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Old => "old",
            Self::New => "new",
        }
    }
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
