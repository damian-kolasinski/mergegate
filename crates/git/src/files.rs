//! The files of a branch, for telling which languages a repo uses.

use std::path::Path;

use mergegate_core::git::GitError;

pub(crate) fn files(repo: &Path, branch: &str) -> Result<Vec<String>, GitError> {
    let failed = |e: &dyn std::fmt::Display| {
        GitError::Failed(format!(
            "listing the files of {branch} in {} failed: {e}",
            repo.display()
        ))
    };
    let repo = gix::open_opts(repo, gix::open::Options::isolated()).map_err(|e| failed(&e))?;
    let tree = repo
        .find_reference(&format!("refs/heads/{branch}"))
        .map_err(|e| failed(&e))?
        .peel_to_commit()
        .map_err(|e| failed(&e))?
        .tree()
        .map_err(|e| failed(&e))?;
    let mut recorder = gix::traverse::tree::Recorder::default();
    tree.traverse()
        .breadthfirst(&mut recorder)
        .map_err(|e| failed(&e))?;
    Ok(recorder
        .records
        .into_iter()
        .filter(|entry| entry.mode.is_blob())
        .map(|entry| entry.filepath.to_string())
        .collect())
}
