use std::path::PathBuf;

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
        None => repo
            .root
            .file_name()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{FakeGit, FakeStore};

    fn request(path: &str) -> AddProject {
        AddProject {
            path: path.into(),
            ..AddProject::default()
        }
    }

    fn added(result: Result<Added, ProjectError>) -> Project {
        match result.unwrap() {
            Added::Project(project) => project,
            Added::Decide(d) => panic!("asked {d:?}"),
        }
    }

    #[test]
    fn registers_a_repo_with_derived_settings_and_syncs_staging() {
        let git = FakeGit::default().repo("/src/My App", &["main", "wip"], Some("wip"));
        let mut store = FakeStore::default();
        let project = added(add(&mut store, &git, request("/src/My App/sub/dir")));
        assert_eq!(project.name.as_str(), "my-app");
        assert_eq!(project.backend, Backend::Local);
        assert_eq!(project.target, "main");
        assert_eq!(project.upstream, PathBuf::from("/src/My App"));
        assert_eq!(store.projects().unwrap(), [project]);
        assert_eq!(
            git.synced(),
            [(
                "my-app".to_owned(),
                PathBuf::from("/src/My App"),
                "main".to_owned()
            )]
        );
    }

    #[test]
    fn asks_for_the_target_without_registering_anything() {
        let git = FakeGit::default().repo("/src/app", &["dev", "prod"], None);
        let mut store = FakeStore::default();
        let result = add(&mut store, &git, request("/src/app")).unwrap();
        assert!(matches!(result, Added::Decide(d) if d.flag == "--target"));
        assert!(store.projects().unwrap().is_empty());
        assert!(git.synced().is_empty());

        let project = added(add(
            &mut store,
            &git,
            AddProject {
                target: Some("prod".into()),
                ..request("/src/app")
            },
        ));
        assert_eq!(project.target, "prod");
    }

    #[test]
    fn refuses_what_the_agent_can_fix_before_asking_the_operator() {
        let git = FakeGit::default()
            .repo("/src/app", &["dev", "prod"], None)
            .repo("/other/app", &["main"], None)
            .repo("/src/empty", &[], None);
        let mut store = FakeStore::default();
        added(add(&mut store, &git, request("/other/app")));

        let err = |r: AddProject, store: &mut FakeStore| add(store, &git, r).unwrap_err();
        assert!(matches!(
            err(request("/src/app"), &mut store),
            ProjectError::NameTaken(n) if n.as_str() == "app"
        ));
        assert!(matches!(
            err(request("/other/app/x"), &mut store),
            ProjectError::AlreadyRegistered { project, .. } if project.as_str() == "app"
        ));
        let bad_name = AddProject {
            name: Some("Bad Name".into()),
            ..request("/src/app")
        };
        assert!(matches!(
            err(bad_name, &mut store),
            ProjectError::InvalidName(_)
        ));
        let github = AddProject {
            backend: Some(Backend::Github),
            ..request("/src/app")
        };
        assert!(matches!(
            err(github, &mut store),
            ProjectError::BackendUnavailable(Backend::Github)
        ));
        assert!(matches!(
            err(request("/src/empty"), &mut store),
            ProjectError::NoCommits(_)
        ));
        assert!(matches!(
            err(request("/nowhere"), &mut store),
            ProjectError::NotARepo(_)
        ));
    }

    #[test]
    fn a_failed_staging_sync_registers_nothing() {
        let git = FakeGit::default()
            .repo("/src/app", &["main"], None)
            .failing_sync();
        let mut store = FakeStore::default();
        assert!(matches!(
            add(&mut store, &git, request("/src/app")).unwrap_err(),
            ProjectError::Git(_)
        ));
        assert!(store.projects().unwrap().is_empty());
    }
}
