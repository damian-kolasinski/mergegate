use super::*;
use crate::fake::{FakeGit, FakeStore};
use crate::project::{AddProject, add};

#[test]
fn finds_projects_by_name_and_by_path() {
    let git =
        FakeGit::default()
            .repo("/src/app", &["main"], None)
            .repo("/src/other", &["main"], None);
    let mut store = FakeStore::default();
    let request = AddProject {
        path: "/src/app".into(),
        ..AddProject::default()
    };
    add(&mut store, &git, request).unwrap();

    assert_eq!(find(&store, "app").unwrap().upstream, Path::new("/src/app"));
    assert!(matches!(find(&store, "nope"), Err(ProjectError::NotFound(n)) if n == "nope"));
    assert!(matches!(
        find(&store, "Not A Name"),
        Err(ProjectError::NotFound(_))
    ));

    assert_eq!(
        at(&store, &git, Path::new("/src/app/deep/dir"))
            .unwrap()
            .name
            .as_str(),
        "app"
    );
    assert!(matches!(
        at(&store, &git, Path::new("/src/other")),
        Err(ProjectError::NoProjectAt(_))
    ));
    assert!(matches!(
        at(&store, &git, Path::new("/tmp")),
        Err(ProjectError::NotARepo(_))
    ));
}
