//! The checks behind `mg doctor` that only the hub can run.

use api::doctor::{Check, CheckStatus};

use crate::git::{self, Git, GitError};
use crate::project::{Project, ProjectStore};
use crate::store::StoreError;

pub fn checks(store: &impl ProjectStore, git: &impl Git) -> Result<Vec<Check>, StoreError> {
    let mut checks = vec![git_version(git)];
    let projects = store.projects()?;
    if projects.is_empty() {
        checks.push(check(
            "projects",
            CheckStatus::Warn,
            "no projects registered",
            Some("mg project add PATH".into()),
        ));
    }
    checks.extend(projects.iter().map(|p| project(git, p)));
    Ok(checks)
}

fn git_version(git: &impl Git) -> Check {
    const FIX: &str = "install git 2.38 or newer";
    let (major, minor) = git::MIN_VERSION;
    let output = match git.version() {
        Ok(output) => output,
        Err(e) => return check("git", CheckStatus::Fail, e.to_string(), Some(FIX.into())),
    };
    let version = output.trim().trim_start_matches("git version ");
    match git::parse_version(&output) {
        Some(found) if found >= git::MIN_VERSION => {
            check("git", CheckStatus::Ok, format!("git {version}"), None)
        }
        Some(_) => check(
            "git",
            CheckStatus::Fail,
            format!("git {version} is older than {major}.{minor}"),
            Some(FIX.into()),
        ),
        None => check(
            "git",
            CheckStatus::Warn,
            format!("can't read a version from {:?}", output.trim()),
            None,
        ),
    }
}

fn project(git: &impl Git, project: &Project) -> Check {
    let name = format!("project {}", project.name);
    let fail = |message: String, fix: String| check(&name, CheckStatus::Fail, message, Some(fix));
    let upstream = project.upstream.display();
    let set = |key: &str, value: &str| format!("mg project set {} {key} {value}", project.name);

    let repo = match git.inspect(&project.upstream) {
        Ok(repo) => repo,
        Err(GitError::NotARepo(_)) => {
            return fail(
                format!("{upstream} is no longer a git repository"),
                set("upstream", "PATH"),
            );
        }
        Err(e) => return fail(e.to_string(), set("upstream", "PATH")),
    };
    if !repo.has_branch(&project.target) {
        return fail(
            format!("{upstream} has no branch {}", project.target),
            set("target", "BRANCH"),
        );
    }
    // Setting the target again re-syncs the staging repo.
    let resync = set("target", &project.target);
    match git.staging_has(&project.name, &project.target) {
        Ok(true) => check(
            &name,
            CheckStatus::Ok,
            format!(
                "{} · {} · {upstream}",
                project.backend.as_str(),
                project.target
            ),
            None,
        ),
        Ok(false) => fail(
            format!("the staging repo has no {}", project.target),
            resync,
        ),
        Err(e) => fail(format!("reading the staging repo failed: {e}"), resync),
    }
}

fn check(
    name: &str,
    status: CheckStatus,
    message: impl Into<String>,
    fix: Option<String>,
) -> Check {
    Check {
        name: name.into(),
        status,
        message: message.into(),
        fix,
    }
}

#[cfg(test)]
mod tests {
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
}
