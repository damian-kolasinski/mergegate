use mergegate_core::github::GithubRepo;

use super::*;

fn project(name: &str, upstream: &str) -> Project {
    Project {
        name: ProjectName::parse(name).unwrap(),
        backend: Backend::Local,
        target: "main".into(),
        upstream: upstream.into(),
        github: None,
        bots: Vec::new(),
    }
}

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("hub.db")).unwrap();
    (dir, store)
}

#[test]
fn stores_and_finds_projects() {
    let (_dir, mut store) = store();
    let app = project("app", "/src/app");
    let api = project("api", "/src/api");
    store.insert_project(&app).unwrap();
    store.insert_project(&api).unwrap();

    assert_eq!(store.projects().unwrap(), [api.clone(), app.clone()]);
    assert_eq!(store.project(&app.name).unwrap(), Some(app.clone()));
    assert_eq!(
        store.project_with_upstream(Path::new("/src/api")).unwrap(),
        Some(api)
    );
    assert_eq!(
        store.project(&ProjectName::parse("nope").unwrap()).unwrap(),
        None
    );
    assert_eq!(
        store.project_with_upstream(Path::new("/src/nope")).unwrap(),
        None
    );
}

#[test]
fn updates_a_projects_settings() {
    let (_dir, mut store) = store();
    let mut app = project("app", "/src/app");
    store.insert_project(&app).unwrap();
    app.target = "release".into();
    app.upstream = "/src/fork".into();
    store.update_project(&app).unwrap();
    assert_eq!(store.projects().unwrap(), [app.clone()]);

    app.backend = Backend::Github;
    app.github = GithubRepo::parse("acme/app");
    app.bots = vec!["codex".into(), "claude[bot]".into()];
    store.update_project(&app).unwrap();
    assert_eq!(store.projects().unwrap(), [app.clone()]);
    // Leaving github keeps the repository for the next switch back.
    app.backend = Backend::Local;
    store.update_project(&app).unwrap();
    assert_eq!(store.projects().unwrap(), [app]);

    assert!(store.update_project(&project("nope", "/x")).is_err());
}

#[test]
fn a_github_project_needs_its_repository() {
    let (_dir, mut store) = store();
    let app = Project {
        backend: Backend::Github,
        ..project("app", "/src/app")
    };
    assert!(store.insert_project(&app).is_err());
}

#[test]
fn keeps_names_and_upstreams_unique() {
    let (_dir, mut store) = store();
    store.insert_project(&project("app", "/src/app")).unwrap();
    assert!(store.insert_project(&project("app", "/src/b")).is_err());
    assert!(store.insert_project(&project("b", "/src/app")).is_err());
}

#[test]
fn reports_a_corrupt_row() {
    let (_dir, store) = store();
    store
        .conn
        .execute(
            "INSERT INTO projects (name, backend, target, upstream) \
                 VALUES ('Bad Name', 'local', 'main', '/x')",
            [],
        )
        .unwrap();
    let err = store.projects().unwrap_err();
    assert!(err.to_string().contains("Bad Name"), "{err}");
}
