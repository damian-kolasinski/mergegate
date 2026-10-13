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
fn names_a_bare_repo_beside_its_worktrees_after_its_folder() {
    let git = FakeGit::default()
        .repo("/src/app/.bare", &["main"], None)
        .repo("/src/api.git", &["main"], None);
    let mut store = FakeStore::default();
    let project = added(add(&mut store, &git, request("/src/app/.bare")));
    assert_eq!(project.name.as_str(), "app");
    let project = added(add(&mut store, &git, request("/src/api.git")));
    assert_eq!(project.name.as_str(), "api");
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
