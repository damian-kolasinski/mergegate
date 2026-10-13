use std::path::Path;

use mergegate_core::git::{GitError, Repo};

pub(crate) fn inspect(path: &Path) -> Result<Repo, GitError> {
    let failed = |e: &dyn std::fmt::Display| {
        GitError::Failed(format!(
            "reading the repo at {} failed: {e}",
            path.display()
        ))
    };
    // Isolated: only the repo's own config applies, as for the git binary.
    let found = gix::discover_opts(path, Default::default(), gix::open::Options::isolated())
        .map_err(|_| GitError::NotARepo(path.to_owned()))?;
    // From a linked worktree, the repo that owns it.
    let repo = found.main_repo().map_err(|e| failed(&e))?;

    let root = match repo.workdir() {
        Some(dir) if !repo.is_bare() => dir,
        _ => repo.git_dir(),
    };
    let root = std::fs::canonicalize(root).map_err(|e| failed(&e))?;

    let mut branches = repo
        .references()
        .map_err(|e| failed(&e))?
        .local_branches()
        .map_err(|e| failed(&e))?
        .map(|r| r.map(|r| r.name().shorten().to_string()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| failed(&e))?;
    branches.sort();
    let head = repo
        .head_name()
        .map_err(|e| failed(&e))?
        .map(|name| name.shorten().to_string());

    Ok(Repo {
        root,
        branches,
        head,
    })
}
