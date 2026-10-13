//! Changing a project's settings after setup (F15).

use std::path::PathBuf;

use super::{Backend, Project, ProjectError, ProjectStore, find};
use crate::decision::Decision;
use crate::git::Git;
use crate::item::ItemStore;

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

#[derive(Debug)]
pub enum Changed {
    Project(Project),
    /// Nothing changed; re-run with `--confirm`.
    Decide(Decision),
}

/// Applies `setting` to the project named `name`. Settings that point at a
/// branch are checked against the repo, and the staging repo follows them.
///
/// A new backend or target applies to new items only. While open items keep
/// the old one, nothing changes until the caller `confirm`s, having been
/// told which items those are.
pub fn set<S: ProjectStore + ItemStore>(
    store: &mut S,
    git: &impl Git,
    name: &str,
    setting: Setting,
    confirm: bool,
) -> Result<Changed, ProjectError> {
    let mut project = find(store, name)?;
    match setting {
        Setting::Backend(Backend::Local) => return Ok(Changed::Project(project)),
        Setting::Backend(backend) => return Err(ProjectError::BackendUnavailable(backend)),
        Setting::Target(target) => {
            let repo = git.inspect(&project.upstream)?;
            if !repo.has_branch(&target) {
                return Err(ProjectError::UnknownBranch {
                    branch: target,
                    upstream: repo.root,
                });
            }
            let decision = keeping_target(store, &project, &target)?;
            if !confirm && let Some(decision) = decision {
                return Ok(Changed::Decide(decision));
            }
            project.target = target;
        }
        Setting::Upstream(path) => {
            let repo = git.inspect(&path)?;
            if let Some(other) = store.project_with_upstream(&repo.root)? {
                if other.name == project.name {
                    return Ok(Changed::Project(project));
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
    Ok(Changed::Project(project))
}

/// What to ask when open items would stay on another target than the one
/// new items get. An open item's own branch can't become the target: staging
/// mirrors the target from the upstream over what the worker pushed.
fn keeping_target(
    store: &impl ItemStore,
    project: &Project,
    target: &str,
) -> Result<Option<Decision>, ProjectError> {
    let open = store.open_items(Some(&project.name))?;
    if let Some(item) = open.iter().find(|item| item.branch == target) {
        return Err(ProjectError::BranchUnderReview {
            branch: target.into(),
            item: item.id.to_string(),
        });
    }
    let staying: Vec<String> = open
        .into_iter()
        .filter(|item| item.target != target)
        .map(|item| format!("{} ({} onto {})", item.id, item.branch, item.target))
        .collect();
    let keep = match staying.len() {
        0 => return Ok(None),
        1 => "1 open item keeps its target until it closes",
        _ => &format!(
            "{} open items keep their targets until they close",
            staying.len()
        ),
    };
    Ok(Some(Decision {
        question: format!(
            "{keep}: {}. New items will target {target}. Change it?",
            staying.join(", ")
        ),
        flag: "--confirm".into(),
        options: Vec::new(),
        default: None,
    }))
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
