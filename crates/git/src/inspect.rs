use std::path::Path;

use mergegate_core::git::{GitError, Repo};

pub(crate) fn inspect(path: &Path) -> Result<Repo, GitError> {
    let failed = |e: &dyn std::fmt::Display| {
        GitError::Failed(format!(
            "reading the repo at {} failed: {e}",
            path.display()
        ))
    };
    let start = match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => path.parent().unwrap_or(path),
        Ok(_) => path,
        Err(_) => return Err(GitError::NotARepo(path.to_owned())),
    };
    // Isolated: only the repo's own config applies, as for the git binary.
    let found = gix::discover_opts(start, Default::default(), gix::open::Options::isolated())
        .map_err(|e| {
            use gix::discover::upwards::Error as Upwards;
            match e.downcast_any_ref::<Upwards>() {
                Some(
                    Upwards::NoGitRepository { .. }
                    | Upwards::NoGitRepositoryWithinCeiling { .. }
                    | Upwards::NoGitRepositoryWithinFs { .. },
                ) => GitError::NotARepo(path.to_owned()),
                // Anything else means a repo is there but can't be read, e.g. a
                // corrupt `.git` or missing permissions; say so instead.
                _ => failed(&e),
            }
        })?;
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
