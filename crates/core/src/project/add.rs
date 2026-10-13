use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use api::decision::NeedsDecision;

use super::target::{Target, choose};
use super::{Backend, Project, ProjectError, ProjectName, ProjectStore};
use crate::git::Git;

#[derive(Debug, Clone, Default)]
pub struct AddProject {
    /// Any path inside the repo, on the hub's machine.
    pub path: PathBuf,
    pub name: Option<String>,
    pub backend: Option<Backend>,
    pub target: Option<String>,
}

#[derive(Debug)]
pub enum Added {
    Project(Project),
    /// Nothing was registered; re-run with the decision's flag set.
    Decide(NeedsDecision),
}

/// Registers the repo containing `request.path` and creates its staging repo.
/// Problems the agent can fix itself are reported before the operator is
/// asked anything.
pub fn add(
    store: &mut impl ProjectStore,
    git: &impl Git,
    request: AddProject,
) -> Result<Added, ProjectError> {
    let backend = request.backend.unwrap_or(Backend::Local);
    if backend != Backend::Local {
        return Err(ProjectError::BackendUnavailable(backend));
    }
    let repo = git.inspect(&request.path)?;
    if repo.branches.is_empty() {
        return Err(ProjectError::NoCommits(repo.root));
    }
    if let Some(existing) = store.project_with_upstream(&repo.root)? {
        return Err(ProjectError::AlreadyRegistered {
            project: existing.name,
            upstream: repo.root,
        });
    }
    let name = match request.name {
        Some(name) => ProjectName::parse(&name)?,
        None => naming_dir(&repo.root)
            .and_then(|dir| ProjectName::derive(&dir.to_string_lossy()))
            .ok_or_else(|| ProjectError::NoName(repo.root.clone()))?,
    };
    if store.project(&name)?.is_some() {
        return Err(ProjectError::NameTaken(name));
    }
    let target = match choose(&repo, request.target.as_deref())? {
        Target::Chosen(target) => target,
        Target::Decide(decision) => return Ok(Added::Decide(decision)),
    };

    git.sync_staging(&name, &repo.root, &target)?;
    let project = Project {
        name,
        backend,
        target,
        upstream: repo.root,
    };
    store.insert_project(&project)?;
    Ok(Added::Project(project))
}

/// The folder a repo is named after. A bare repo kept beside its worktrees,
/// as in `app/.bare` or `app/.git`, is named after the folder holding it.
fn naming_dir(root: &Path) -> Option<&OsStr> {
    let dir = root.file_name()?;
    if dir.as_encoded_bytes().starts_with(b".") {
        return root.parent()?.file_name();
    }
    Some(dir)
}

#[cfg(test)]
#[path = "add_tests.rs"]
mod tests;
