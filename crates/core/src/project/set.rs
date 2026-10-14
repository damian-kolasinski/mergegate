//! Changing a project's settings after setup (F15).

use std::path::PathBuf;

use super::add::parse_github;
use super::{Backend, Project, ProjectError, ProjectStore, fetch_from, find};
use crate::decision::Decision;
use crate::git::Git;
use crate::github::{Detected, Github, GithubRepo, detect};
use crate::item::{Item, ItemBackend, ItemStore};

/// The keys `mg project set` accepts.
pub const KEYS: &[&str] = &["backend", "target", "upstream", "github"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    Backend(Backend),
    Target(String),
    /// Any path inside the repo, on the hub's machine.
    Upstream(PathBuf),
    Github(GithubRepo),
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
            "github" => parse_github(value).map(Self::Github),
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
/// told which items those are. Moving to `github` takes the repository from
/// the `github` setting or the remotes, and GitHub must let the operator
/// push to it.
pub fn set<S: ProjectStore + ItemStore>(
    store: &mut S,
    git: &impl Git,
    github: &impl Github,
    name: &str,
    setting: Setting,
    confirm: bool,
) -> Result<Changed, ProjectError> {
    let mut project = find(store, name)?;
    match setting {
        Setting::Backend(backend) if backend == project.backend => {
            return Ok(Changed::Project(project));
        }
        Setting::Backend(backend) => {
            if backend == Backend::Github {
                let github_repo = match project.github.clone() {
                    Some(github_repo) => github_repo,
                    None => match detect(&git.inspect(&project.upstream)?.remotes) {
                        Detected::One(github_repo) => github_repo,
                        _ => return Err(ProjectError::NoGithubSet(project.name)),
                    },
                };
                github.check_access(&github_repo)?;
                project.github = Some(github_repo);
            }
            let decision = keeping_backend(store, &project, backend)?;
            if !confirm && let Some(decision) = decision {
                return Ok(Changed::Decide(decision));
            }
            project.backend = backend;
        }
        Setting::Github(github_repo) => {
            if project.github.as_ref() == Some(&github_repo) {
                return Ok(Changed::Project(project));
            }
            let publishing: Vec<String> = store
                .open_items(Some(&project.name))?
                .into_iter()
                .filter(|item| item.backend.kind() == Backend::Github)
                .map(|item| item.id.to_string())
                .collect();
            if let Some(current) = &project.github
                && !publishing.is_empty()
            {
                return Err(ProjectError::GithubInUse {
                    repo: current.to_string(),
                    items: publishing,
                });
            }
            if project.backend == Backend::Github {
                github.check_access(&github_repo)?;
            }
            project.github = Some(github_repo);
            if project.backend == Backend::Local {
                // Nothing reads it until the backend moves to github.
                store.update_project(&project)?;
                return Ok(Changed::Project(project));
            }
        }
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
    let upstream = fetch_from(&project, project.backend, github)?;
    git.sync_staging(&project.name, &upstream, &project.target)?;
    store.update_project(&project)?;
    Ok(Changed::Project(project))
}

/// What to ask when open items would stay on another backend than the one
/// new items get.
fn keeping_backend(
    store: &impl ItemStore,
    project: &Project,
    backend: Backend,
) -> Result<Option<Decision>, ProjectError> {
    let staying: Vec<Item> = store
        .open_items(Some(&project.name))?
        .into_iter()
        .filter(|item| item.backend.kind() != backend)
        .collect();
    let listed: Vec<String> = staying
        .iter()
        .map(|item| match &item.backend {
            ItemBackend::Github { pull } => format!("{} ({}, {pull})", item.id, item.branch),
            ItemBackend::Local => format!("{} ({})", item.id, item.branch),
        })
        .collect();
    let keep = match staying.len() {
        0 => return Ok(None),
        1 => format!(
            "1 open item keeps the {} backend until it closes",
            project.backend.as_str()
        ),
        n => format!(
            "{n} open items keep the {} backend until they close",
            project.backend.as_str()
        ),
    };
    let lands = match (backend, &project.github) {
        (Backend::Github, Some(github_repo)) => format!("through pull requests on {github_repo}"),
        _ => format!("on {} in {}", project.target, project.upstream.display()),
    };
    Ok(Some(Decision {
        question: format!(
            "{keep}: {}. New items will land {lands}. Switch?",
            listed.join(", ")
        ),
        flag: "--confirm".into(),
        options: Vec::new(),
        default: None,
    }))
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
