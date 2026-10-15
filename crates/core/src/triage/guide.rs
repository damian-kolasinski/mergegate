//! The agent's review guide (F11): Markdown whose `## Files` list orders the
//! files to review and says why each matters. It can't hide a file or bring
//! a trusted one back; only the operator's rules sort files into tiers.

use super::{Tier, TriagedBy};
use crate::item::FileStat;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Guide {
    /// In the guide's order, each path once.
    pub files: Vec<GuideFile>,
    /// Whether the text has a `## Files` heading at all.
    listed: bool,
    /// Lines of the list that name no path, or name one again.
    problems: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuideFile {
    pub path: String,
    pub reason: Option<String>,
}

impl Guide {
    /// Reads the bullets of `text`'s `## Files` section, as in
    /// `` - `src/auth.rs` — the token check moved here `` or numbered as
    /// `` 1. `src/auth.rs` ``. Anything else in the text is for the operator
    /// to read and ignored here.
    pub fn parse(text: &str) -> Self {
        let mut guide = Self::default();
        let mut in_files = false;
        let mut fenced = false;
        // The bullet indented lines continue: `Some(None)` under one that
        // was refused, whose wrapped text belongs to no file.
        let mut open: Option<Option<usize>> = None;
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            if let Some(title) = heading(trimmed) {
                in_files = trimmed.starts_with("## ") && title.eq_ignore_ascii_case("files");
                guide.listed |= in_files;
                open = None;
                continue;
            }
            if !in_files || trimmed.is_empty() {
                continue;
            }
            // A wrapped reason or a sub-bullet adds to the bullet above.
            if line.starts_with([' ', '\t'])
                && let Some(current) = open
            {
                if let Some(file) = current.map(|i| &mut guide.files[i]) {
                    let reason = file.reason.get_or_insert_with(String::new);
                    if !reason.is_empty() {
                        reason.push(' ');
                    }
                    reason.push_str(trimmed.trim_end());
                }
                continue;
            }
            let Some(bullet) = bullet(trimmed) else {
                open = None;
                continue;
            };
            open = Some(None);
            let Some((path, rest)) = bullet
                .strip_prefix('`')
                .and_then(|rest| rest.split_once('`'))
                .filter(|(path, _)| !path.trim().is_empty())
            else {
                guide
                    .problems
                    .push(format!("\"{}\" names no `path`", trimmed.trim_end()));
                continue;
            };
            let path = path.trim().to_owned();
            if guide.files.iter().any(|f| f.path == path) {
                guide
                    .problems
                    .push(format!("`{path}` is listed twice; its first place counts"));
                continue;
            }
            let reason = ["—", "–", "--", "-", ":"]
                .iter()
                .fold(rest.trim(), |rest, separator| {
                    rest.strip_prefix(separator).unwrap_or(rest)
                })
                .trim();
            open = Some(Some(guide.files.len()));
            guide.files.push(GuideFile {
                path,
                reason: (!reason.is_empty()).then(|| reason.to_owned()),
            });
        }
        guide
    }

    /// Where the guide puts `path` among the files to review.
    pub fn position(&self, path: &str) -> Option<usize> {
        self.files.iter().position(|f| f.path == path)
    }

    pub fn reason(&self, path: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|f| f.path == path)?
            .reason
            .as_deref()
    }

    /// What the agent should fix in the guide it sent with a revision of
    /// `files`: paths the revision doesn't change or the rules trust, and
    /// lines that name no path.
    pub fn warnings(&self, text: &str, files: &[FileStat]) -> Vec<String> {
        let mut warnings = Vec::new();
        if !self.listed && !text.trim().is_empty() {
            warnings
                .push("the guide has no `## Files` list, so the files stay in path order".into());
        } else if self.listed && self.files.is_empty() {
            warnings.push(
                "the guide's `## Files` list names no `path` before the next heading, \
                 so the files stay in path order"
                    .into(),
            );
        }
        warnings.extend(self.problems.iter().cloned());
        for listed in &self.files {
            let Some(file) = files.iter().find(|f| f.file.path == listed.path) else {
                warnings.push(format!("`{}` isn't changed in this revision", listed.path));
                continue;
            };
            if file.triage.tier == Tier::Trusted {
                let by = match &file.triage.by {
                    Some(TriagedBy::Rule { id, .. }) => format!(" by {id}"),
                    _ => String::new(),
                };
                warnings.push(format!(
                    "`{}` is trusted{by}, and the guide only orders the files to review",
                    listed.path
                ));
            }
        }
        warnings
    }
}

/// What follows a list item's marker: `-`, `*`, `+`, or a number with `.`
/// or `)`, since an ordered list is the natural way to write an order.
fn bullet(line: &str) -> Option<&str> {
    let rest = ["- ", "* ", "+ "]
        .iter()
        .find_map(|marker| line.strip_prefix(marker));
    rest.or_else(|| {
        let digits = line.bytes().take_while(u8::is_ascii_digit).count();
        let rest = &line[digits..];
        (digits > 0)
            .then(|| rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")))
            .flatten()
    })
    .map(str::trim_start)
}

/// The title of an ATX heading, as in `## Files`.
fn heading(line: &str) -> Option<&str> {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    let rest = &line[hashes..];
    ((1..=6).contains(&hashes) && (rest.is_empty() || rest.starts_with([' ', '\t'])))
        .then(|| rest.trim().trim_end_matches('#').trim())
}

#[cfg(test)]
#[path = "guide_tests.rs"]
mod tests;
