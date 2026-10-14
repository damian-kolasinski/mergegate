//! Attributes triage reads from a commit's tree.

use std::ffi::OsStr;

use mergegate_core::git::GitError;
use mergegate_core::project::ProjectName;
use mergegate_core::triage::TriageGit;

use crate::HubGit;
use crate::command::run_with_input;

impl TriageGit for HubGit {
    fn generated(
        &self,
        project: &ProjectName,
        commit: &str,
        paths: &[&str],
    ) -> Result<Vec<bool>, GitError> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let doing = "reading linguist-generated";
        let source = format!("--source={commit}");
        let mut input = Vec::new();
        for path in paths {
            input.extend_from_slice(path.as_bytes());
            input.push(0);
        }
        // `--source` needs git 2.40; staging is bare, so without it there's no
        // tree to read attributes from.
        let output = run_with_input(
            &self.home,
            doing,
            [
                self.git_dir(project).as_os_str(),
                OsStr::new("check-attr"),
                OsStr::new("--stdin"),
                OsStr::new("-z"),
                OsStr::new(&source),
                OsStr::new("linguist-generated"),
            ],
            &input,
        )?;
        parse(&output, paths.len())
            .ok_or_else(|| GitError::Failed(format!("{doing} failed: can't parse {output:?}")))
    }
}

/// Parses `git check-attr -z` for one attribute: `path NUL attribute NUL
/// value NUL` per path, in input order. `set` and `true` mean generated, as
/// in linguist; `false`, `unset` and `unspecified` don't.
fn parse(output: &str, count: usize) -> Option<Vec<bool>> {
    let fields: Vec<&str> = output.strip_suffix('\0')?.split('\0').collect();
    if fields.len() != count * 3 {
        return None;
    }
    Some(
        fields
            .chunks(3)
            .map(|f| matches!(f[2], "set" | "true"))
            .collect(),
    )
}

#[cfg(test)]
#[path = "triage_tests.rs"]
mod tests;
