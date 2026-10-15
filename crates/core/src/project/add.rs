use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::languages::{self, Chosen, Languages};
use super::target::{Target, choose};
use super::{Backend, Project, ProjectError, ProjectName, ProjectStore, fetch_from};
use crate::decision::{Decision, DecisionOption};
use crate::git::{Git, Repo};
use crate::github::{Detected, Github, GithubRepo, detect};

#[derive(Debug, Clone, Default)]
pub struct AddProject {
    /// Any path inside the repo, on the hub's machine.
    pub path: PathBuf,
    pub name: Option<String>,
    pub backend: Option<Backend>,
    pub target: Option<String>,
    /// `owner/name` on GitHub, when the remotes don't settle it.
    pub github: Option<String>,
    pub languages: Languages,
}

#[derive(Debug)]
pub enum Added {
    /// Registered; the caller installs `languages`, the packs it asked for.
    Project {
        project: Project,
        languages: Vec<String>,
    },
    /// Nothing was registered; re-run with the decision's flag set.
    Decide(Decision),
}

/// Registers the repo containing `request.path` and creates its staging repo.
/// Problems the agent can fix itself are reported before the operator is
/// asked anything. A repo with a GitHub remote asks for the backend; the
/// `github` one is only set up once GitHub confirms the operator can push.
pub fn add(
    store: &mut impl ProjectStore,
    git: &impl Git,
    github: &impl Github,
    request: AddProject,
) -> Result<Added, ProjectError> {
    let named = request.github.as_deref().map(parse_github).transpose()?;
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
    let detected = match &named {
        Some(repo) => Detected::One(repo.clone()),
        None => detect(&repo.remotes),
    };
    let backend = match (request.backend, &named, &detected) {
        (Some(backend), _, _) => backend,
        (None, Some(_), _) => Backend::Github,
        (None, None, Detected::None) => Backend::Local,
        (None, None, _) => return Ok(Added::Decide(choosing_backend(&repo, &detected))),
    };
    let github_repo = match (backend, detected) {
        (Backend::Local, _) => named,
        (Backend::Github, Detected::One(github_repo)) => Some(github_repo),
        (Backend::Github, Detected::Several(repos)) => {
            return Ok(Added::Decide(choosing_github(&repo, repos)));
        }
        (Backend::Github, Detected::None) => return Err(ProjectError::NoGithubRepo(repo.root)),
    };
    let target = match choose(&repo, request.target.as_deref())? {
        Target::Chosen(target) => target,
        Target::Decide(decision) => return Ok(Added::Decide(decision)),
    };
    let languages = match languages::choose(git, &repo.root, &target, &request.languages)? {
        Chosen::Install(names) => names,
        Chosen::Decide(decision) => return Ok(Added::Decide(decision)),
    };

    if backend == Backend::Github
        && let Some(github_repo) = &github_repo
    {
        github.check_access(github_repo)?;
    }
    let project = Project {
        name,
        backend,
        target,
        upstream: repo.root,
        github: github_repo,
        bots: Vec::new(),
    };
    let upstream = fetch_from(&project, backend, github)?;
    git.sync_staging(&project.name, &upstream, &project.target)?;
    store.insert_project(&project)?;
    Ok(Added::Project { project, languages })
}

pub(super) fn parse_github(value: &str) -> Result<GithubRepo, ProjectError> {
    GithubRepo::parse(value).ok_or_else(|| ProjectError::InvalidValue {
        key: "github",
        value: value.into(),
        expected: "OWNER/REPO",
    })
}

fn choosing_backend(repo: &Repo, detected: &Detected) -> Decision {
    let on_github = match detected {
        Detected::One(github_repo) => github_repo.to_string(),
        _ => "GitHub".into(),
    };
    Decision {
        question: format!(
            "{} has a GitHub remote. Should its items land through pull requests on {on_github}, \
             or straight on the target branch of this repo?",
            repo.root.display()
        ),
        flag: "--backend".into(),
        options: vec![
            DecisionOption {
                value: Backend::Github.as_str().into(),
                description: Some(format!(
                    "push each item's branch to {on_github}, open its pull request \
                     and squash-merge it there"
                )),
            },
            DecisionOption {
                value: Backend::Local.as_str().into(),
                description: Some("squash-merge into the target branch on this machine".into()),
            },
        ],
        default: Some(Backend::Github.as_str().into()),
    }
}

fn choosing_github(repo: &Repo, repos: Vec<GithubRepo>) -> Decision {
    Decision {
        question: format!(
            "The remotes of {} point at several GitHub repositories, and none is origin. \
             Which one should items publish to?",
            repo.root.display()
        ),
        flag: "--github".into(),
        options: repos
            .into_iter()
            .map(|github_repo| DecisionOption {
                value: github_repo.to_string(),
                description: None,
            })
            .collect(),
        default: None,
    }
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
