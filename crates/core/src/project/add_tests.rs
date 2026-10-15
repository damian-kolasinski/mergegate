use super::*;
use crate::fake::{FakeGit, FakeGithub, FakeStore, pack};
use crate::github::{GithubError, GithubRepo};

fn request(path: &str) -> AddProject {
    AddProject {
        path: path.into(),
        ..AddProject::default()
    }
}

fn added(result: Result<Added, ProjectError>) -> Project {
    match result.unwrap() {
        Added::Project { project, .. } => project,
        Added::Decide(d) => panic!("asked {d:?}"),
    }
}

#[test]
fn registers_a_repo_with_derived_settings_and_syncs_staging() {
    let git = FakeGit::default().repo("/src/My App", &["main", "wip"], Some("wip"));
    let mut store = FakeStore::default();
    let project = added(add(
        &mut store,
        &git,
        &FakeGithub::default(),
        request("/src/My App/sub/dir"),
    ));
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
    let project = added(add(
        &mut store,
        &git,
        &FakeGithub::default(),
        request("/src/app/.bare"),
    ));
    assert_eq!(project.name.as_str(), "app");
    let project = added(add(
        &mut store,
        &git,
        &FakeGithub::default(),
        request("/src/api.git"),
    ));
    assert_eq!(project.name.as_str(), "api");
}

#[test]
fn asks_for_the_target_without_registering_anything() {
    let git = FakeGit::default().repo("/src/app", &["dev", "prod"], None);
    let mut store = FakeStore::default();
    let result = add(
        &mut store,
        &git,
        &FakeGithub::default(),
        request("/src/app"),
    )
    .unwrap();
    assert!(matches!(result, Added::Decide(d) if d.flag == "--target"));
    assert!(store.projects().unwrap().is_empty());
    assert!(git.synced().is_empty());

    let project = added(add(
        &mut store,
        &git,
        &FakeGithub::default(),
        AddProject {
            target: Some("prod".into()),
            ..request("/src/app")
        },
    ));
    assert_eq!(project.target, "prod");
}

#[test]
fn asks_for_the_languages_without_registering_anything() {
    let git = FakeGit::default()
        .repo("/src/app", &["main"], None)
        .files("/src/app", &["src/main.rs"]);
    let mut store = FakeStore::default();
    let languages = |chosen: Option<Vec<String>>| Languages {
        known: vec![pack("rs", "1")],
        installed: Vec::new(),
        chosen,
    };
    let result = add(
        &mut store,
        &git,
        &FakeGithub::default(),
        AddProject {
            languages: languages(None),
            ..request("/src/app")
        },
    )
    .unwrap();
    assert!(matches!(result, Added::Decide(d) if d.flag == "--languages"));
    assert!(store.projects().unwrap().is_empty());
    assert!(git.synced().is_empty());

    let result = add(
        &mut store,
        &git,
        &FakeGithub::default(),
        AddProject {
            languages: languages(Some(vec!["rs".into()])),
            ..request("/src/app")
        },
    )
    .unwrap();
    assert!(matches!(result, Added::Project { languages, .. } if languages == ["rs"]));
}

#[test]
fn refuses_what_the_agent_can_fix_before_asking_the_operator() {
    let git = FakeGit::default()
        .repo("/src/app", &["dev", "prod"], None)
        .repo("/other/app", &["main"], None)
        .repo("/src/empty", &[], None);
    let mut store = FakeStore::default();
    added(add(
        &mut store,
        &git,
        &FakeGithub::default(),
        request("/other/app"),
    ));

    let err = |r: AddProject, store: &mut FakeStore| {
        add(store, &git, &FakeGithub::default(), r).unwrap_err()
    };
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
        name: Some("other".into()),
        backend: Some(Backend::Github),
        ..request("/src/app")
    };
    assert!(matches!(
        err(github, &mut store),
        ProjectError::NoGithubRepo(_)
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
        add(
            &mut store,
            &git,
            &FakeGithub::default(),
            request("/src/app")
        )
        .unwrap_err(),
        ProjectError::Git(_)
    ));
    assert!(store.projects().unwrap().is_empty());
}

fn on_github() -> FakeGit {
    FakeGit::default()
        .repo("/src/app", &["main"], None)
        .remote("/src/app", "origin", "git@github.com:acme/app.git")
        .remote("/src/app", "fork", "https://github.com/me/app")
}

#[test]
fn asks_for_the_backend_of_a_repo_on_github() {
    let git = on_github();
    let mut store = FakeStore::default();
    let github = FakeGithub::default();
    let Added::Decide(decision) = add(&mut store, &git, &github, request("/src/app")).unwrap()
    else {
        panic!("registered without asking");
    };
    assert_eq!(decision.flag, "--backend");
    assert_eq!(decision.default.as_deref(), Some("github"));
    assert!(
        decision.question.contains("acme/app"),
        "{}",
        decision.question
    );
    assert!(store.projects().unwrap().is_empty());

    let project = added(add(
        &mut store,
        &git,
        &github,
        AddProject {
            backend: Some(Backend::Github),
            ..request("/src/app")
        },
    ));
    assert_eq!(project.backend, Backend::Github);
    assert_eq!(project.github, GithubRepo::parse("acme/app"));
    let synced = &git.synced()[0];
    assert_eq!(synced.1, PathBuf::from("https://github.com/acme/app.git"));
}

#[test]
fn a_named_github_repo_picks_the_github_backend() {
    let git = FakeGit::default().repo("/src/app", &["main"], None);
    let mut store = FakeStore::default();
    let named = AddProject {
        github: Some("acme/app".into()),
        ..request("/src/app")
    };
    let project = added(add(&mut store, &git, &FakeGithub::default(), named));
    assert_eq!(
        (project.backend, project.github),
        (Backend::Github, GithubRepo::parse("acme/app"))
    );

    let bad = AddProject {
        github: Some("not a repo".into()),
        ..request("/src/app")
    };
    let err = add(&mut store, &git, &FakeGithub::default(), bad).unwrap_err();
    assert!(
        matches!(err, ProjectError::InvalidValue { key: "github", .. }),
        "{err:?}"
    );
}

#[test]
fn a_local_project_registers_without_asking_github() {
    let git = on_github();
    let mut store = FakeStore::default();
    let local = AddProject {
        backend: Some(Backend::Local),
        ..request("/src/app")
    };
    let github = FakeGithub::default().logged_out();
    let project = added(add(&mut store, &git, &github, local));
    assert_eq!((project.backend, project.github), (Backend::Local, None));
    assert_eq!(git.synced()[0].1, PathBuf::from("/src/app"));
}

#[test]
fn asks_which_repo_when_several_remotes_but_no_origin_are_on_github() {
    let git = FakeGit::default()
        .repo("/src/app", &["main"], None)
        .remote("/src/app", "upstream", "git@github.com:acme/app.git")
        .remote("/src/app", "mine", "git@github.com:me/app.git");
    let mut store = FakeStore::default();
    let github_backend = AddProject {
        backend: Some(Backend::Github),
        ..request("/src/app")
    };
    let Added::Decide(decision) =
        add(&mut store, &git, &FakeGithub::default(), github_backend).unwrap()
    else {
        panic!("registered without asking");
    };
    assert_eq!(decision.flag, "--github");
    let options: Vec<&str> = decision.options.iter().map(|o| o.value.as_str()).collect();
    assert_eq!(options, ["acme/app", "me/app"]);
}

#[test]
fn registers_nothing_github_wont_let_the_operator_push_to() {
    let git = on_github();
    let mut store = FakeStore::default();
    let github_backend = || AddProject {
        backend: Some(Backend::Github),
        ..request("/src/app")
    };
    let denying = FakeGithub::default().denying("acme/app");
    let err = add(&mut store, &git, &denying, github_backend()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "acme/app doesn't exist on GitHub, or the account the hub signs in as can't push to it"
    );
    let logged_out = FakeGithub::default().logged_out();
    let err = add(&mut store, &git, &logged_out, github_backend()).unwrap_err();
    assert!(
        matches!(err, ProjectError::Github(GithubError::NotLoggedIn(_))),
        "{err:?}"
    );
    assert!(store.projects().unwrap().is_empty());
    assert!(git.synced().is_empty());
}
