//! The agent's review guide (F11): Markdown whose `## Files` list orders the
//! files to review and says why each matters. It can't hide a file or bring
//! a trusted one back; only the operator's rules sort files into tiers.

use super::{Tier, TriagedBy};
use crate::item::FileStat;
use crate::review::CommentId;

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
        // The bullet indented lines continue, with the column its text
        // starts at: `None` under one that was refused, whose wrapped text
        // belongs to no file.
        let mut open: Option<(Option<usize>, usize)> = None;
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            if heading(trimmed).is_some() {
                in_files = files_heading(trimmed);
                guide.listed |= in_files;
                open = None;
                continue;
            }
            if !in_files || trimmed.is_empty() {
                continue;
            }
            let indent = indent(line);
            let item = bullet(trimmed);
            // A wrapped reason, or a sub-bullet indented to the text of the
            // bullet above, adds to it. As in Markdown, a bullet indented
            // less is that bullet's sibling, but only one naming a path
            // counts as such: agents often indent a note on a numbered item
            // by two spaces, short of its text.
            if indent > 0
                && let Some((current, text)) = open
                && (indent >= text || item.and_then(named).is_none())
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
            let Some(bullet) = item else {
                open = None;
                continue;
            };
            let text = indent + trimmed.len() - bullet.len();
            open = Some((None, text));
            let Some((path, rest)) = named(bullet) else {
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
            open = Some((Some(guide.files.len()), text));
            guide.files.push(GuideFile {
                path,
                reason: (!reason.is_empty()).then(|| reason.to_owned()),
            });
        }
        guide
    }

    /// What `text` says before its `## Files` list, trimmed: the change's
    /// intent, without the walk through its files. All of it without a list.
    pub fn intent(text: &str) -> &str {
        let mut fenced = false;
        let mut at = 0;
        for line in text.split_inclusive('\n') {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                fenced = !fenced;
            } else if !fenced && files_heading(trimmed) {
                return text[..at].trim();
            }
            at += line.len();
        }
        text.trim()
    }

    /// A warning when `text`'s intent names one of `comments`, the item's
    /// comment IDs, or one of its revisions up to `revision` as `r2`: the
    /// intent becomes the squash commit's body, where neither means anything.
    /// A comment counts anywhere, as agents quote its ID as code. A revision
    /// counts only lowercase in prose: Cloudflare's `R2`, a register or a
    /// variable in code, or a segment of a path, URL or link target are no
    /// revision.
    pub fn review_warning(text: &str, comments: &[CommentId], revision: u32) -> Option<String> {
        let mut named: Vec<&str> = Vec::new();
        let mut fenced = false;
        for line in Self::intent(text).lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            let indented = line.starts_with("    ") || line.starts_with('\t');
            // Every other piece between backticks is a code span.
            for (piece, code) in line.split('`').zip([false, true].into_iter().cycle()) {
                let code = code || fenced || indented;
                for token in piece.split_whitespace() {
                    let path = token.contains('/');
                    for word in token.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
                        let comment =
                            CommentId::parse(word).is_some_and(|id| comments.contains(&id));
                        let revised = !code
                            && !path
                            && word
                                .strip_prefix('r')
                                .filter(|number| !number.starts_with('0'))
                                .and_then(|number| number.parse::<u32>().ok())
                                .is_some_and(|number| number <= revision);
                        if (comment || revised) && !named.contains(&word) {
                            named.push(word);
                        }
                    }
                }
            }
        }
        if named.is_empty() {
            return None;
        }
        let named: Vec<String> = named.iter().map(|word| format!("`{word}`")).collect();
        Some(format!(
            "the guide's intent names {} from the review, but it becomes the commit message \
             when the item merges: describe the change as a whole",
            named.join(", ")
        ))
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

/// The `path` a bullet's text starts with, and what follows it.
fn named(bullet: &str) -> Option<(&str, &str)> {
    bullet
        .strip_prefix('`')
        .and_then(|rest| rest.split_once('`'))
        .filter(|(path, _)| !path.trim().is_empty())
}

/// The column `line`'s text starts at, a tab reaching the next multiple of
/// four as in Markdown.
fn indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .fold(0, |column, c| match c {
            '\t' => column + 4 - column % 4,
            _ => column + 1,
        })
}

/// Whether `line`, its indent trimmed, opens the list of files.
fn files_heading(line: &str) -> bool {
    line.starts_with("## ")
        && heading(line).is_some_and(|title| title.eq_ignore_ascii_case("files"))
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
