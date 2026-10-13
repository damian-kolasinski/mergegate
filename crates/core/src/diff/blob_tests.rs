use super::*;
use crate::fake::{FakeGit, FakeStore};
use crate::project::{Backend, Project, ProjectName};

#[test]
fn reads_a_blob_of_a_known_project_only() {
    let mut store = FakeStore::default();
    store
        .insert_project(&Project {
            name: ProjectName::parse("app").unwrap(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
        })
        .unwrap();
    let git = FakeGit::default();
    assert_eq!(
        blob(&store, &git, "app", "abc").unwrap(),
        Some(b"abc".to_vec())
    );
    assert!(matches!(
        blob(&store, &git, "web", "abc"),
        Err(ProjectError::NotFound(name)) if name == "web"
    ));
    assert_eq!(git.blobs_read(), ["abc"]);
}
