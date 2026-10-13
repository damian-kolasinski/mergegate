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
            "backend" => match value {
                "github" => Ok(Self::Backend(Backend::Github)),
                "local" => Ok(Self::Backend(Backend::Local)),
                _ => Err(ProjectError::InvalidValue {
                    key: "backend",
                    value: value.into(),
                    expected: "github, local",
                }),
            },
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
mod tests {
    use std::path::Path;

    use super::*;
    use crate::fake::{FakeGit, FakeStore};
    use crate::project::{AddProject, add};

    fn setup(git: &FakeGit) -> FakeStore {
        let mut store = FakeStore::default();
        let request = AddProject {
            path: "/src/app".into(),
            ..AddProject::default()
        };
        add(&mut store, git, request).unwrap();
        store
    }

    fn git() -> FakeGit {
        FakeGit::default()
            .repo("/src/app", &["main", "release"], None)
            .repo("/src/fork", &["main"], None)
            .repo("/src/bare.git", &["release"], None)
    }

    #[test]
    fn parses_settings() {
        assert_eq!(
            Setting::parse("backend", "local").unwrap(),
            Setting::Backend(Backend::Local)
        );
        assert_eq!(
            Setting::parse("target", "dev").unwrap(),
            Setting::Target("dev".into())
        );
        assert!(matches!(
            Setting::parse("backend", "gitlab"),
            Err(ProjectError::InvalidValue { key: "backend", .. })
        ));
        let err = Setting::parse("colour", "red").unwrap_err();
        assert_eq!(
            err.to_string(),
            "unknown setting colour; settings are backend, target, upstream"
        );
    }

    #[test]
    fn changes_the_target_and_resyncs_staging() {
        let git = git();
        let mut store = setup(&git);
        let project = set(&mut store, &git, "app", Setting::Target("release".into())).unwrap();
        assert_eq!(project.target, "release");
        assert_eq!(store.projects().unwrap(), [project]);
        assert_eq!(git.synced().last().unwrap().2, "release");

        let err = set(&mut store, &git, "app", Setting::Target("nope".into())).unwrap_err();
        assert!(matches!(err, ProjectError::UnknownBranch { .. }));
        assert_eq!(store.projects().unwrap()[0].target, "release");
    }

    #[test]
    fn moves_the_upstream_only_to_a_repo_with_the_target() {
        let git = git();
        let mut store = setup(&git);
        let project = set(
            &mut store,
            &git,
            "app",
            Setting::Upstream("/src/fork/x".into()),
        )
        .unwrap();
        assert_eq!(project.upstream, Path::new("/src/fork"));
        assert_eq!(git.synced().last().unwrap().1, PathBuf::from("/src/fork"));

        let err = set(
            &mut store,
            &git,
            "app",
            Setting::Upstream("/src/bare.git".into()),
        )
        .unwrap_err();
        assert!(matches!(err, ProjectError::UnknownBranch { branch, .. } if branch == "main"));

        // Pointing it at its own repo again changes nothing.
        let synced = git.synced().len();
        set(
            &mut store,
            &git,
            "app",
            Setting::Upstream("/src/fork".into()),
        )
        .unwrap();
        assert_eq!(git.synced().len(), synced);
    }

    #[test]
    fn refuses_another_projects_repo_and_unavailable_backends() {
        let git = git();
        let mut store = setup(&git);
        let request = AddProject {
            path: "/src/fork".into(),
            ..AddProject::default()
        };
        add(&mut store, &git, request).unwrap();
        let err = set(
            &mut store,
            &git,
            "app",
            Setting::Upstream("/src/fork".into()),
        )
        .unwrap_err();
        assert!(
            matches!(err, ProjectError::AlreadyRegistered { project, .. } if project.as_str() == "fork")
        );

        let err = set(&mut store, &git, "app", Setting::Backend(Backend::Github)).unwrap_err();
        assert!(matches!(
            err,
            ProjectError::BackendUnavailable(Backend::Github)
        ));
        assert!(set(&mut store, &git, "app", Setting::Backend(Backend::Local)).is_ok());
        assert!(matches!(
            set(&mut store, &git, "nope", Setting::Backend(Backend::Local)),
            Err(ProjectError::NotFound(_))
        ));
    }
}
