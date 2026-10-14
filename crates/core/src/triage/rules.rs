//! Rules as the operator writes them, checked and compiled once.

use std::fmt;

use globset::{GlobBuilder, GlobMatcher};
use regex::Regex;

use super::Tier;

/// `R<n>`. IDs aren't reused, so a file's stored triage keeps naming the
/// rule it was triaged by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(u32);

impl RuleId {
    pub fn new(number: u32) -> Self {
        Self(number)
    }

    pub fn number(self) -> u32 {
        self.0
    }

    /// `R` and a number from 1, without leading zeros, so each rule has one
    /// spelling.
    pub fn parse(id: &str) -> Option<Self> {
        let digits = id.strip_prefix('R')?;
        if digits.is_empty()
            || digits.starts_with('0')
            || !digits.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        digits.parse().ok().map(Self)
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "R{}", self.0)
    }
}

/// What a change did to a file. A file can be several: a generated file can
/// also be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Added,
    Deleted,
    Renamed,
    /// Changed in place: neither added, deleted nor renamed.
    Modified,
    /// `linguist-generated` in the base's `.gitattributes`.
    Generated,
}

impl Kind {
    pub const ALL: [Self; 5] = [
        Self::Added,
        Self::Deleted,
        Self::Renamed,
        Self::Modified,
        Self::Generated,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Deleted => "deleted",
            Self::Renamed => "renamed",
            Self::Modified => "modified",
            Self::Generated => "generated",
        }
    }

    pub fn parse(kind: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == kind)
    }
}

/// A rule matches a file when every matcher it sets does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: RuleId,
    pub tier: Tier,
    /// A glob over the repo-relative path; `*` stays within a directory and
    /// `**/` spans any number of them.
    pub path: Option<String>,
    pub kind: Option<Kind>,
    /// Lines added and removed together, at most.
    pub max_lines: Option<u64>,
    /// A regex over the added and removed lines, without their context: a
    /// `review` rule needs one of them to match, a `trusted` one all.
    pub diff_matches: Option<String>,
    pub reason: String,
}

impl Rule {
    /// The matchers, as the UI and `mg rules list` show them.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(path) = &self.path {
            parts.push(format!("`{path}`"));
        }
        if let Some(kind) = self.kind {
            parts.push(kind.as_str().to_owned());
        }
        if let Some(max) = self.max_lines {
            parts.push(format!("≤ {max} lines"));
        }
        if let Some(regex) = &self.diff_matches {
            parts.push(format!("diff matches `{regex}`"));
        }
        parts.join(", ")
    }
}

/// An ordered rule list that passed [`Rules::new`]'s checks, first match
/// wins.
#[derive(Debug, Clone, Default)]
pub struct Rules {
    compiled: Vec<Compiled>,
}

#[derive(Debug, Clone)]
pub(super) struct Compiled {
    pub rule: Rule,
    pub path: Option<GlobMatcher>,
    pub diff: Option<Regex>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    #[error("{0} appears twice")]
    DuplicateId(RuleId),
    #[error("{0} has no matcher; give it a path, kind, max_lines or diff_matches")]
    NoMatcher(RuleId),
    #[error("{0} has no reason")]
    NoReason(RuleId),
    #[error("{id}'s path {path:?} isn't a valid glob: {error}")]
    InvalidPath {
        id: RuleId,
        path: String,
        error: String,
    },
    #[error("{id}'s diff_matches {regex:?} isn't a valid regex: {error}")]
    InvalidRegex {
        id: RuleId,
        regex: String,
        error: String,
    },
}

impl Rules {
    pub fn new(rules: Vec<Rule>) -> Result<Self, RuleError> {
        let mut compiled: Vec<Compiled> = Vec::with_capacity(rules.len());
        for rule in rules {
            let id = rule.id;
            if compiled.iter().any(|c| c.rule.id == id) {
                return Err(RuleError::DuplicateId(id));
            }
            if rule.path.is_none()
                && rule.kind.is_none()
                && rule.max_lines.is_none()
                && rule.diff_matches.is_none()
            {
                return Err(RuleError::NoMatcher(id));
            }
            if rule.reason.trim().is_empty() {
                return Err(RuleError::NoReason(id));
            }
            let path = match &rule.path {
                Some(path) => Some(
                    GlobBuilder::new(path)
                        .literal_separator(true)
                        .build()
                        .map_err(|e| RuleError::InvalidPath {
                            id,
                            path: path.clone(),
                            error: e.kind().to_string(),
                        })?
                        .compile_matcher(),
                ),
                None => None,
            };
            let diff = match &rule.diff_matches {
                Some(regex) => Some(Regex::new(regex).map_err(|e| RuleError::InvalidRegex {
                    id,
                    regex: regex.clone(),
                    error: e.to_string(),
                })?),
                None => None,
            };
            compiled.push(Compiled { rule, path, diff });
        }
        Ok(Self { compiled })
    }

    pub fn rules(&self) -> impl ExactSizeIterator<Item = &Rule> {
        self.compiled.iter().map(|c| &c.rule)
    }

    pub fn is_empty(&self) -> bool {
        self.compiled.is_empty()
    }

    pub(super) fn compiled(&self) -> &[Compiled] {
        &self.compiled
    }
}

impl PartialEq for Rules {
    fn eq(&self, other: &Self) -> bool {
        self.rules().eq(other.rules())
    }
}

impl Eq for Rules {}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
