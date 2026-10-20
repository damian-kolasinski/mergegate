use std::collections::BTreeMap;

use super::place::{ancestors, place};
use super::{Entry, ModuleGit};
use crate::git::GitError;
use crate::item::FileStat;
use crate::project::ProjectName;

/// Places each of a revision's `files` in its module, by the folders around
/// it at `base` and `head` together: a deleted file's are only at the base.
pub fn locate(
    git: &impl ModuleGit,
    project: &ProjectName,
    base: &str,
    head: &str,
    files: &mut [FileStat],
) -> Result<(), GitError> {
    let dirs = ancestors(files.iter().map(|f| f.file.path.as_str()));
    let dirs: Vec<&str> = dirs.iter().map(String::as_str).collect();
    let mut listings: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
    for commit in [base, head] {
        for (dir, entries) in dirs.iter().zip(git.listings(project, commit, &dirs)?) {
            listings
                .entry((*dir).to_owned())
                .or_default()
                .extend(entries);
        }
    }
    for file in files {
        file.placement = place(&file.file.path, &listings);
    }
    Ok(())
}

#[cfg(test)]
#[path = "locate_tests.rs"]
mod tests;
