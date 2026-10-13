use super::*;
use crate::fake::{FakeGit, FakeStore};
use crate::project::{AddProject, add};

fn statuses(checks: &[Check]) -> Vec<(&str, CheckStatus, Option<&str>)> {
    checks
        .iter()
        .map(|c| (c.name.as_str(), c.status, c.fix.as_deref()))
        .collect()
}

#[test]
fn healthy_setup_passes() {
    let git = FakeGit::default().repo("/src/app", &["main"], None);
    let mut store = FakeStore::default();
    let request = AddProject {
        path: "/src/app".into(),
        ..AddProject::default()
    };
    add(&mut store, &git, request).unwrap();
    let checks = checks(&store, &git).unwrap();
    assert_eq!(
        statuses(&checks),
        [
            ("git", CheckStatus::Ok, None),
            ("project app", CheckStatus::Ok, None)
        ]
    );
    assert_eq!(checks[0].message, "git 2.50.1");
    assert_eq!(checks[1].message, "local · main · /src/app");
}

#[test]
fn no_projects_is_a_warning() {
    let checks = checks(&FakeStore::default(), &FakeGit::default()).unwrap();
    assert_eq!(
        statuses(&checks)[1],
        ("projects", CheckStatus::Warn, Some("mg project add PATH"))
    );
}

#[test]
fn old_or_missing_git_fails() {
    let cases = [
        (Some("git version 2.37.9"), CheckStatus::Fail),
        (Some("git version 2.38.0"), CheckStatus::Ok),
        (Some("something else"), CheckStatus::Warn),
        (None, CheckStatus::Fail),
    ];
    for (version, expected) in cases {
        let git = FakeGit::default().version(version);
        let checks = checks(&FakeStore::default(), &git).unwrap();
        assert_eq!(checks[0].status, expected, "{version:?}");
    }
}

#[test]
fn broken_projects_name_their_fix() {
    let git = FakeGit::default()
        .repo("/src/gone", &["main"], None)
        .repo("/src/moved", &["main"], None)
        .repo("/src/unsynced", &["main"], None);
    let mut store = FakeStore::default();
    for path in ["/src/gone", "/src/moved", "/src/unsynced"] {
        let request = AddProject {
            path: path.into(),
            ..AddProject::default()
        };
        add(&mut store, &git, request).unwrap();
    }
    let git = git
        .without_repo("/src/gone")
        .repo("/src", &["main"], None)
        .repo("/src/moved", &["trunk"], None)
        .without_staging("unsynced");

    let checks = checks(&store, &git).unwrap();
    assert_eq!(
        statuses(&checks)[1..],
        [
            (
                "project gone",
                CheckStatus::Fail,
                Some("mg project set gone upstream PATH")
            ),
            (
                "project moved",
                CheckStatus::Fail,
                Some("mg project set moved target BRANCH")
            ),
            (
                "project unsynced",
                CheckStatus::Fail,
                Some("mg project set unsynced target main")
            ),
        ]
    );
    assert_eq!(checks[2].message, "/src/moved has no branch main");
}
