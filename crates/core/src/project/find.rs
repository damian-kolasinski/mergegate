use std::path::Path;

use super::{Project, ProjectError, ProjectName, ProjectStore};
use crate::git::Git;

pub fn find(store: &impl ProjectStore, name: &str) -> Result<Project, ProjectError> {
    let not_found = || ProjectError::NotFound(name.into());
    let name = ProjectName::parse(name).map_err(|_| not_found())?;
    store.project(&name)?.ok_or_else(not_found)
}

/// The project whose repo contains `path`, from any of its worktrees.
pub fn at(store: &impl ProjectStore, git: &impl Git, path: &Path) -> Result<Project, ProjectError> {
    let repo = git.inspect(path)?;
    store
        .project_with_upstream(&repo.root)?
        .ok_or(ProjectError::NoProjectAt(repo.root))
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
