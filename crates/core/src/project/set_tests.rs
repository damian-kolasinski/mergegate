use std::path::Path;

use super::*;
use crate::fake::{FakeGit, FakeGithub, FakeStore, commit};
use crate::github::{GithubError, GithubRepo, Pull};
use crate::item::{ItemBackend, ItemId, ItemState, NewItem, NewRevision};
use crate::project::{AddProject, ProjectName, add};

/// `set` without `--confirm`, expecting the change to apply.
fn change(
    store: &mut FakeStore,
    git: &FakeGit,
    name: &str,
    setting: Setting,
) -> Result<Project, ProjectError> {
    match set(store, git, &FakeGithub::default(), name, setting, false)? {
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
        backend: ItemBackend::Local,
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
        backend: Some(Backend::Local),
        ..AddProject::default()
    };
    add(&mut store, git, &FakeGithub::default(), request).unwrap();
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
        "unknown setting colour; settings are backend, target, upstream, github"
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
    add(&mut store, &git, &FakeGithub::default(), request).unwrap();
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
    assert!(matches!(err, ProjectError::NoGithubSet(_)));
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
    let Changed::Decide(decision) = set(
        &mut store,
        &git,
        &FakeGithub::default(),
        "app",
        release(),
        false,
    )
    .unwrap() else {
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

    let Changed::Project(project) = set(
        &mut store,
        &git,
        &FakeGithub::default(),
        "app",
        release(),
        true,
    )
    .unwrap() else {
        panic!("asked again");
    };
    assert_eq!(project.target, "release");

    // Only items on another target than the new one are listed.
    let docs = open_item(&mut store, "docs", "release");
    store.set_state(&docs, ItemState::ChangesRequested);
    let main = || Setting::Target("main".into());
    let Changed::Decide(decision) = set(
        &mut store,
        &git,
        &FakeGithub::default(),
        "app",
        main(),
        false,
    )
    .unwrap() else {
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

#[test]
fn refuses_an_open_items_branch_as_the_target() {
    let git = git();
    let mut store = setup(&git);
    open_item(&mut store, "release", "main");
    let release = || Setting::Target("release".into());
    for confirm in [false, true] {
        let err = set(
            &mut store,
            &git,
            &FakeGithub::default(),
            "app",
            release(),
            confirm,
        )
        .unwrap_err();
        assert!(
            matches!(&err, ProjectError::BranchUnderReview { branch, item }
                if branch == "release" && item == "app#1"),
            "{err:?}"
        );
    }
    assert_eq!(store.projects().unwrap()[0].target, "main");
}

fn on_github() -> FakeGit {
    git().remote("/src/app", "origin", "git@github.com:acme/app.git")
}

/// An open item with pull request 7.
fn github_item(store: &mut FakeStore, branch: &str) -> ItemId {
    let item = NewItem {
        project: ProjectName::parse("app").unwrap(),
        branch: branch.into(),
        title: branch.into(),
        state: ItemState::NeedsReview,
        backend: ItemBackend::Github {
            repo: GithubRepo::parse("acme/app").unwrap(),
            pull: Pull {
                number: 7,
                url: "https://github.com/acme/app/pull/7".into(),
            },
        },
        target: "main".into(),
        revision: NewRevision {
            head: commit(2),
            base: commit(1),
            files: Vec::new(),
        },
    };
    store.insert_item(item).unwrap().id
}

#[test]
fn switches_to_github_through_the_origin_remote() {
    let git = on_github();
    let mut store = setup(&git);
    let project = change(&mut store, &git, "app", Setting::Backend(Backend::Github)).unwrap();
    assert_eq!(
        (project.backend, project.github.clone()),
        (Backend::Github, GithubRepo::parse("acme/app"))
    );
    assert_eq!(store.projects().unwrap(), [project]);
    let synced = git.synced().last().unwrap().1.clone();
    assert_eq!(synced, PathBuf::from("https://github.com/acme/app.git"));

    // Back to local keeps the repository for the next switch.
    let project = change(&mut store, &git, "app", Setting::Backend(Backend::Local)).unwrap();
    assert_eq!(project.github, GithubRepo::parse("acme/app"));
    assert_eq!(git.synced().last().unwrap().1, PathBuf::from("/src/app"));
}

#[test]
fn asks_before_switching_backends_under_open_items() {
    let git = on_github();
    let mut store = setup(&git);
    open_item(&mut store, "login", "main");
    let github = || Setting::Backend(Backend::Github);
    let Changed::Decide(decision) = set(
        &mut store,
        &git,
        &FakeGithub::default(),
        "app",
        github(),
        false,
    )
    .unwrap() else {
        panic!("applied without asking");
    };
    assert_eq!(
        decision.question,
        "1 open item keeps the local backend until it closes: app#1 (login). \
         New items will land through pull requests on acme/app. Switch?"
    );
    assert_eq!(store.projects().unwrap()[0].backend, Backend::Local);
    let Changed::Project(project) = set(
        &mut store,
        &git,
        &FakeGithub::default(),
        "app",
        github(),
        true,
    )
    .unwrap() else {
        panic!("asked again");
    };
    assert_eq!(project.backend, Backend::Github);

    github_item(&mut store, "search");
    let local = || Setting::Backend(Backend::Local);
    let Changed::Decide(decision) = set(
        &mut store,
        &git,
        &FakeGithub::default(),
        "app",
        local(),
        false,
    )
    .unwrap() else {
        panic!("applied without asking");
    };
    assert_eq!(
        decision.question,
        "1 open item keeps the github backend until it closes: app#2 (search, #7). \
         New items will land on main in /src/app. Switch?"
    );
}

#[test]
fn switches_to_github_only_where_the_operator_can_push() {
    let git = on_github();
    let mut store = setup(&git);
    let github = || Setting::Backend(Backend::Github);
    let denying = FakeGithub::default().denying("acme/app");
    let err = set(&mut store, &git, &denying, "app", github(), true).unwrap_err();
    assert!(
        matches!(err, ProjectError::Github(GithubError::NoAccess(_))),
        "{err:?}"
    );
    let project = &store.projects().unwrap()[0];
    assert_eq!(
        (project.backend, project.github.clone()),
        (Backend::Local, None)
    );
}

#[test]
fn sets_the_github_repo_for_new_items() {
    let git = git();
    let mut store = setup(&git);
    let acme = || Setting::Github(GithubRepo::parse("acme/app").unwrap());
    let synced = git.synced().len();
    let project = change(&mut store, &git, "app", acme()).unwrap();
    assert_eq!(
        (project.backend, project.github),
        (Backend::Local, GithubRepo::parse("acme/app"))
    );
    // A local project's staging doesn't fetch from GitHub.
    assert_eq!(git.synced().len(), synced);
    change(&mut store, &git, "app", Setting::Backend(Backend::Github)).unwrap();

    github_item(&mut store, "login");
    let mine = || Setting::Github(GithubRepo::parse("me/app").unwrap());
    let github = FakeGithub::default();
    let Changed::Decide(decision) = set(&mut store, &git, &github, "app", mine(), false).unwrap()
    else {
        panic!("applied without asking");
    };
    assert_eq!(
        decision.question,
        "1 open item keeps its repository until it closes: app#1 (login, acme/app#7). \
         New items will publish to me/app. Change it?"
    );
    assert_eq!(
        store.projects().unwrap()[0].github,
        GithubRepo::parse("acme/app")
    );
    let Changed::Project(project) = set(&mut store, &git, &github, "app", mine(), true).unwrap()
    else {
        panic!("asked again");
    };
    assert_eq!(project.github, GithubRepo::parse("me/app"));
    // Back to the repository the item publishes to: nothing to ask.
    assert!(change(&mut store, &git, "app", acme()).is_ok());
}
