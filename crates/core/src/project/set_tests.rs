use std::path::Path;

use super::*;
use crate::fake::{FakeGit, FakeStore, commit};
use crate::item::{ItemId, ItemState, NewItem, NewRevision};
use crate::project::{AddProject, ProjectName, add};

/// `set` without `--confirm`, expecting the change to apply.
fn change(
    store: &mut FakeStore,
    git: &FakeGit,
    name: &str,
    setting: Setting,
) -> Result<Project, ProjectError> {
    match set(store, git, name, setting, false)? {
        Changed::Project(project) => Ok(project),
        Changed::Decide(decision) => panic!("asked: {decision:?}"),
    }
}

fn open_item(store: &mut FakeStore, branch: &str, target: &str) -> ItemId {
    let item = NewItem {
        project: ProjectName::parse("app").unwrap(),
        branch: branch.into(),
        title: branch.into(),
        state: ItemState::NeedsReview,
        backend: Backend::Local,
        target: target.into(),
        revision: NewRevision {
            head: commit(2),
            base: commit(1),
            files: Vec::new(),
        },
    };
    store.insert_item(item).unwrap().id
}

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
    let project = change(&mut store, &git, "app", Setting::Target("release".into())).unwrap();
    assert_eq!(project.target, "release");
    assert_eq!(store.projects().unwrap(), [project]);
    assert_eq!(git.synced().last().unwrap().2, "release");

    let err = change(&mut store, &git, "app", Setting::Target("nope".into())).unwrap_err();
    assert!(matches!(err, ProjectError::UnknownBranch { .. }));
    assert_eq!(store.projects().unwrap()[0].target, "release");
}

#[test]
fn moves_the_upstream_only_to_a_repo_with_the_target() {
    let git = git();
    let mut store = setup(&git);
    let project = change(
        &mut store,
        &git,
        "app",
        Setting::Upstream("/src/fork/x".into()),
    )
    .unwrap();
    assert_eq!(project.upstream, Path::new("/src/fork"));
    assert_eq!(git.synced().last().unwrap().1, PathBuf::from("/src/fork"));

    let err = change(
        &mut store,
        &git,
        "app",
        Setting::Upstream("/src/bare.git".into()),
    )
    .unwrap_err();
    assert!(matches!(err, ProjectError::UnknownBranch { branch, .. } if branch == "main"));

    // Pointing it at its own repo again changes nothing.
    let synced = git.synced().len();
    change(
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
    let err = change(
        &mut store,
        &git,
        "app",
        Setting::Upstream("/src/fork".into()),
    )
    .unwrap_err();
    assert!(
        matches!(err, ProjectError::AlreadyRegistered { project, .. } if project.as_str() == "fork")
    );

    let err = change(&mut store, &git, "app", Setting::Backend(Backend::Github)).unwrap_err();
    assert!(matches!(
        err,
        ProjectError::BackendUnavailable(Backend::Github)
    ));
    assert!(change(&mut store, &git, "app", Setting::Backend(Backend::Local)).is_ok());
    assert!(matches!(
        change(&mut store, &git, "nope", Setting::Backend(Backend::Local)),
        Err(ProjectError::NotFound(_))
    ));
}

#[test]
fn asks_before_moving_the_target_away_from_open_items() {
    let git = git();
    let mut store = setup(&git);
    open_item(&mut store, "login", "main");
    open_item(&mut store, "search", "main");
    let merged = open_item(&mut store, "old", "main");
    store.set_state(&merged, ItemState::Merged);
    let synced = git.synced().len();

    let release = || Setting::Target("release".into());
    let Changed::Decide(decision) = set(&mut store, &git, "app", release(), false).unwrap() else {
        panic!("applied without asking");
    };
    assert_eq!(decision.flag, "--confirm");
    assert!(decision.options.is_empty());
    assert_eq!(
        decision.question,
        "2 open items keep their targets until they close: app#1 (login onto main), \
         app#2 (search onto main). New items will target release. Change it?"
    );
    assert_eq!(store.projects().unwrap()[0].target, "main");
    assert_eq!(git.synced().len(), synced);

    let Changed::Project(project) = set(&mut store, &git, "app", release(), true).unwrap() else {
        panic!("asked again");
    };
    assert_eq!(project.target, "release");

    // Only items on another target than the new one are listed.
    let docs = open_item(&mut store, "docs", "release");
    store.set_state(&docs, ItemState::ChangesRequested);
    let main = || Setting::Target("main".into());
    let Changed::Decide(decision) = set(&mut store, &git, "app", main(), false).unwrap() else {
        panic!("applied without asking");
    };
    assert_eq!(
        decision.question,
        "1 open item keeps its target until it closes: app#4 (docs onto release). \
         New items will target main. Change it?"
    );
    store.set_state(&docs, ItemState::Abandoned);
    assert_eq!(
        change(&mut store, &git, "app", main()).unwrap().target,
        "main"
    );
}
