//! What Jev reads: a summary of the whole change, then as many whole diffs
//! as fit its budget. A change too large for one request is split, each
//! part with the full summary, so every file is judged against the change as
//! a whole.

use std::fmt::Write;

use mergegate_core::item::FileMode;
use mergegate_core::judge::{JudgeError, Request};
use mergegate_core::triage::Tier;

use crate::secrets::looks_secret;

/// Tokens a state may take. Jev's limit is 32k for the state and its longest
/// question; a question takes a few hundred.
pub(crate) const BUDGET: usize = 30_000;

/// Files a part asks about, bounding the questions' total.
pub(crate) const MAX_FILES: usize = 40;

/// Code runs about 3.5 bytes a token; 3 errs toward smaller parts.
pub(crate) fn tokens(text: &str) -> usize {
    text.len().div_ceil(3)
}

/// One request's state and the files it asks about, as indexes into the
/// request's files.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Part {
    pub state: String,
    pub files: Vec<usize>,
}

/// Files with no diff, or whose diff alone is over the budget, are in no
/// part: they go unjudged rather than judged on a cut diff. So do files whose
/// diff looks like it holds a secret, which only the summary names. Files
/// stay in path order, so a directory's travel together.
pub(crate) fn parts(request: &Request) -> Result<Vec<Part>, JudgeError> {
    let summary = summary(request);
    let room = BUDGET.saturating_sub(tokens(&summary));
    if room == 0 {
        return Err(JudgeError(format!(
            "{} changes too many files for Jev to see at once",
            request.item
        )));
    }
    let mut parts: Vec<(String, Vec<usize>)> = Vec::new();
    for (index, file) in request.files.iter().enumerate() {
        let Some(diff) = file.diff.as_deref().filter(|d| !looks_secret(d)) else {
            continue;
        };
        let section = format!("\n### File {}: {}\n{diff}", index + 1, file.path);
        if tokens(&section) > room {
            continue;
        }
        let fits = parts.last().is_some_and(|(diffs, files)| {
            files.len() < MAX_FILES && tokens(diffs) + tokens(&section) <= room
        });
        if !fits {
            parts.push((String::new(), Vec::new()));
        }
        let (diffs, files) = parts.last_mut().expect("pushed");
        diffs.push_str(&section);
        files.push(index);
    }
    Ok(parts
        .into_iter()
        .map(|(diffs, files)| Part {
            state: format!("{summary}\n## Diffs\n{diffs}"),
            files,
        })
        .collect())
}

/// The title, the agent's guide and a map of every changed file.
fn summary(request: &Request) -> String {
    let mut out = format!("# A code change under review\n\nTitle: {}\n", request.title);
    if let Some(guide) = request.guide.as_deref().filter(|g| !g.trim().is_empty()) {
        let _ = write!(out, "\n## The author's guide\n\n{}\n", guide.trim_end());
    }
    out.push_str("\n## Changed files\n\n");
    for (index, file) in request.files.iter().enumerate() {
        let _ = write!(
            out,
            "{}. {} ({}",
            index + 1,
            file.path,
            file.change.as_str()
        );
        if let Some(old) = &file.old_path {
            let _ = write!(out, " from {old}");
        }
        let kind = file.new_mode.or(file.old_mode);
        match (file.additions, file.deletions) {
            (Some(added), Some(removed)) => {
                let _ = write!(out, ", +{added} -{removed}");
            }
            _ if kind == Some(FileMode::Submodule) => {}
            _ => out.push_str(", binary"),
        }
        // A mode changes no line, and a symlink's one line reads as text.
        if let (Some(old), Some(new)) = (file.old_mode, file.new_mode)
            && old != new
        {
            let _ = write!(out, ", mode {} → {}", old.as_str(), new.as_str());
        }
        match kind {
            Some(FileMode::Symlink) => out.push_str(", a symlink: its line is its target"),
            Some(FileMode::Submodule) => {
                out.push_str(", a submodule: its object is a commit of another repository");
            }
            _ => {}
        }
        if file.generated {
            out.push_str(", generated");
        }
        if file.tier == Tier::Trusted {
            out.push_str(", marked safe to skip by a rule");
        }
        out.push_str(")\n");
    }
    out
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
