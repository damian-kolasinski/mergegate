//! The JSON a judge command reads and writes. It's the command's contract,
//! documented in design §7.4: fields are only ever added.

use mergegate_core::judge::{JudgeError, Request, Verdict};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct Input<'a> {
    item: String,
    revision: u32,
    title: &'a str,
    guide: Option<&'a str>,
    files: Vec<InputFile<'a>>,
}

#[derive(Serialize)]
struct InputFile<'a> {
    path: &'a str,
    old_path: Option<&'a str>,
    change: &'static str,
    additions: Option<u64>,
    deletions: Option<u64>,
    /// `review`, or `trusted` when a rule marked it safe to skip.
    tier: &'static str,
    generated: bool,
    /// Unified hunks; null when the change isn't text or is too large.
    diff: Option<&'a str>,
}

pub(crate) fn input(request: &Request) -> Input<'_> {
    Input {
        item: request.item.to_string(),
        revision: request.revision,
        title: &request.title,
        guide: request.guide.as_deref(),
        files: request
            .files
            .iter()
            .map(|f| InputFile {
                path: &f.path,
                old_path: f.old_path.as_deref(),
                change: f.change.as_str(),
                additions: f.additions,
                deletions: f.deletions,
                tier: f.tier.as_str(),
                generated: f.generated,
                diff: f.diff.as_deref(),
            })
            .collect(),
    }
}

#[derive(Deserialize)]
struct Output {
    files: Vec<OutputFile>,
}

#[derive(Deserialize)]
struct OutputFile {
    path: String,
    /// 1 to 100; a fraction is rounded.
    importance: f64,
    reason: Option<String>,
    /// 0 to 1, for a trusted file only.
    promote: Option<f64>,
}

/// The verdicts in `stdout`, as given: the pass drops and clamps.
pub(crate) fn verdicts(stdout: &[u8]) -> Result<Vec<Verdict>, JudgeError> {
    let output: Output = serde_json::from_slice(stdout).map_err(|e| {
        JudgeError(format!(
            "the judge command's output isn't {{\"files\": [{{\"path\", \"importance\"}}]}}: {e}"
        ))
    })?;
    Ok(output
        .files
        .into_iter()
        .map(|f| Verdict {
            path: f.path,
            importance: f.importance.round().clamp(1.0, 100.0) as u8,
            reason: f.reason,
            promote: f.promote,
        })
        .collect())
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
