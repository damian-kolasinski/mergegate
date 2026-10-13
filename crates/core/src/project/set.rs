//! Changing a project's settings after setup (F15).
//!
//! TODO(PR 3): once items exist, changing `backend` or `target` lists the
//! open items that keep their settings in a `needs_decision` before applying.

use std::path::PathBuf;

use super::{Backend, Project, ProjectError, ProjectStore, find};
use crate::git::Git;

/// The keys `mg project set` accepts.
pub const KEYS: &[&str] = &["backend", "target", "upstream"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    Backend(Backend),
    Target(String),
    /// Any path inside the repo, on the hub's machine.
    Upstream(PathBuf),
}

impl Setting {
    pub fn parse(key: &str, value: &str) -> Result<Self, ProjectError> {
        match key {
            "backend" => {
                Backend::parse(value)
                    .map(Self::Backend)
                    .ok_or_else(|| ProjectError::InvalidValue {
                        key: "backend",
                        value: value.into(),
                        expected: "github, local",
                    })
            }
            "target" => Ok(Self::Target(value.into())),
            "upstream" => Ok(Self::Upstream(value.into())),
            _ => Err(ProjectError::UnknownSetting(key.into())),
        }
    }
}

/// Applies `setting` to the project named `name`. Settings that point at a
/// branch are checked against the repo, and the staging repo follows them.
pub fn set(
    store: &mut impl ProjectStore,
    git: &impl Git,
    name: &str,
    setting: Setting,
) -> Result<Project, ProjectError> {
    let mut project = find(store, name)?;
    match setting {
        Setting::Backend(Backend::Local) => return Ok(project),
        Setting::Backend(backend) => return Err(ProjectError::BackendUnavailable(backend)),
        Setting::Target(target) => {
            let repo = git.inspect(&project.upstream)?;
            if !repo.has_branch(&target) {
                return Err(ProjectError::UnknownBranch {
                    branch: target,
                    upstream: repo.root,
                });
            }
            project.target = target;
        }
        Setting::Upstream(path) => {
            let repo = git.inspect(&path)?;
            if let Some(other) = store.project_with_upstream(&repo.root)? {
                if other.name == project.name {
                    return Ok(project);
                }
                return Err(ProjectError::AlreadyRegistered {
                    project: other.name,
                    upstream: repo.root,
                });
            }
            if !repo.has_branch(&project.target) {
                return Err(ProjectError::UnknownBranch {
                    branch: project.target,
                    upstream: repo.root,
                });
            }
            project.upstream = repo.root;
        }
    }
    git.sync_staging(&project.name, &project.upstream, &project.target)?;
    store.update_project(&project)?;
    Ok(project)
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
