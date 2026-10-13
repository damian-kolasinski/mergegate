//! The checks behind `mg doctor` that only the hub can run.

use crate::git::{self, Git, GitError};
use crate::project::{Project, ProjectStore};
use crate::store::StoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

/// One thing `mg doctor` verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// Stable, machine-readable name, e.g. `git` or `project app`.
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    /// The command that fixes a `Warn` or `Fail`, when there is one.
    pub fix: Option<String>,
}

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
    let (major, minor) = git::MIN_VERSION;
    let fix = || Some(format!("install git {major}.{minor} or newer"));
    let output = match git.version() {
        Ok(output) => output,
        Err(e) => return check("git", CheckStatus::Fail, e.to_string(), fix()),
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
            fix(),
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
        Ok(repo) if repo.root == project.upstream => repo,
        // A repo found further up (say, a dotfiles repo in the home
        // directory) is not the project's.
        Ok(_) | Err(GitError::NotARepo(_)) => {
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
#[path = "doctor_tests.rs"]
mod tests;
