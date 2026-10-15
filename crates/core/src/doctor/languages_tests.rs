use super::*;
use crate::fake::{FakeGit, FakePackSource, installed, pack};
use crate::highlight::PackSource;
use crate::project::{Backend, ProjectName};

fn project(name: &str) -> Project {
    Project {
        name: ProjectName::parse(name).unwrap(),
        backend: Backend::Local,
        target: "main".into(),
        upstream: format!("/src/{name}").into(),
        github: None,
        bots: Vec::new(),
    }
}

fn git() -> FakeGit {
    FakeGit::default()
        .repo("/src/app", &["main"], None)
        .files("/src/app", &["main.rs", "Cargo.toml"])
        .repo("/src/docs", &["main"], None)
        .files("/src/docs", &["README.md"])
}

fn statuses(checks: &[Check]) -> Vec<(&str, CheckStatus, &str, Option<&str>)> {
    checks
        .iter()
        .map(|c| {
            let fix = c.fix.as_deref();
            (c.name.as_str(), c.status, c.message.as_str(), fix)
        })
        .collect()
}

#[test]
fn lists_the_installed_packs_and_the_projects_missing_some() {
    let known = [pack("rs", "1"), pack("toml", "1"), pack("md", "1")];
    let source = FakePackSource::with(&[("rs", "1"), ("toml", "1"), ("md", "1")]);
    let checks = languages(
        &git(),
        &[project("app"), project("docs")],
        &known,
        &installed(&[("md", "1")]),
        source.releases(),
    );
    assert_eq!(
        statuses(&checks),
        [
            ("languages", CheckStatus::Ok, "md", None),
            (
                "languages app",
                CheckStatus::Warn,
                "uses rs, toml, shown as plain text",
                Some("mg lang add --detect app")
            ),
        ]
    );
}

#[test]
fn packs_built_for_another_mg_are_a_warning() {
    let known = [pack("md", "2")];
    let source = FakePackSource::with(&[("md", "2")]);
    let checks = languages(
        &git(),
        &[],
        &known,
        &installed(&[("md", "1")]),
        source.releases(),
    );
    assert_eq!(
        statuses(&checks),
        [(
            "languages",
            CheckStatus::Warn,
            "md built for another mg",
            Some("mg lang update")
        )]
    );
}

#[test]
fn without_releases_only_the_installed_packs_are_listed() {
    let known = [pack("rs", "1")];
    let checks = languages(&git(), &[project("app")], &known, &installed(&[]), &[]);
    assert_eq!(
        statuses(&checks),
        [("languages", CheckStatus::Ok, "none installed", None)]
    );
}

#[test]
fn a_project_whose_repo_is_gone_is_left_to_its_project_check() {
    let known = [pack("rs", "1")];
    let source = FakePackSource::with(&[("rs", "1")]);
    let git = git().without_repo("/src/app");
    let checks = languages(
        &git,
        &[project("app")],
        &known,
        &installed(&[]),
        source.releases(),
    );
    assert_eq!(checks.len(), 1);
}
