//! `mg doctor`: every check, and the command that fixes each failure.

use std::path::Path;

use serde_json::{Value, json};
use tempfile::TempDir;
use testkit::{Env, Run, TestRepo};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

/// Outside any git repo, where doctor has no clone to check. `mg` run from
/// the test's own directory would be in mergegate's.
fn elsewhere() -> TempDir {
    tempfile::tempdir().unwrap()
}

fn doctor(env: &Env, dir: &Path, code: i32) -> Value {
    env.mg_at(dir, &["--json", "doctor"])
        .assert_code(code)
        .json()
}

/// Each check's name and status, in order.
fn statuses(doctor: &Value) -> Vec<(String, String)> {
    doctor["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let field = |key: &str| c[key].as_str().unwrap().to_owned();
            (field("name"), field("status"))
        })
        .collect()
}

fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(name, status)| ((*name).to_owned(), (*status).to_owned()))
        .collect()
}

fn check<'a>(doctor: &'a Value, name: &str) -> &'a Value {
    doctor["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no {name} check in {doctor}"))
}

#[test]
fn without_a_hub_doctor_fails_and_says_how_to_start_it() {
    let env = env();
    let dir = elsewhere();
    let doctor = doctor(&env, dir.path(), 1);
    assert_eq!(doctor["ok"], false);
    assert_eq!(
        doctor["checks"][0],
        json!({
            "name": "hub",
            "status": "fail",
            "message": "the hub is not running",
            "fix": "mg hub start",
        })
    );
    // The skill and PATH are checked without a hub; the rest needs one.
    // With no agent installed, the skill has nowhere to go.
    assert_eq!(
        statuses(&doctor),
        pairs(&[("hub", "fail"), ("skill", "ok"), ("path", "ok")])
    );

    let Run { stdout, .. } = env.mg_at(dir.path(), &["doctor"]).assert_code(1);
    assert!(
        stdout.starts_with("FAIL  hub    the hub is not running\n             fix: mg hub start\n"),
        "{stdout}"
    );
}

#[test]
fn a_fresh_hub_passes_with_warnings_until_a_project_and_the_skill_are_added() {
    let env = env();
    std::fs::create_dir(env.user_home().join(".claude")).unwrap();
    env.start_hub();
    let dir = elsewhere();
    let doctor = doctor(&env, dir.path(), 0);
    assert_eq!(doctor["ok"], true);
    assert_eq!(
        statuses(&doctor),
        pairs(&[
            ("hub", "ok"),
            ("ui", "ok"),
            ("git", "ok"),
            ("projects", "warn"),
            ("languages", "ok"),
            ("skill", "warn"),
            ("path", "ok"),
        ])
    );
    assert_eq!(check(&doctor, "projects")["fix"], "mg project add PATH");
    assert_eq!(check(&doctor, "skill")["fix"], "mg skill setup");
}

#[test]
fn the_skill_check_passes_once_set_up_and_warns_once_changed() {
    let env = env();
    std::fs::create_dir(env.user_home().join(".claude")).unwrap();
    env.start_hub();
    let dir = elsewhere();
    env.mg(&["skill", "setup"]).assert_code(0);
    let skill = check(&doctor(&env, dir.path(), 0), "skill").clone();
    assert_eq!(skill["status"], "ok");
    assert_eq!(skill["message"], "set up in Claude Code");

    let stub = env.user_home().join(".claude/skills/mergegate/SKILL.md");
    std::fs::write(
        stub,
        "---\nname: mergegate\nmetadata:\n  mergegate-stub: \"0\"\n---\n",
    )
    .unwrap();
    let skill = check(&doctor(&env, dir.path(), 0), "skill").clone();
    assert_eq!(skill["status"], "warn");
    assert_eq!(
        skill["message"],
        "not set up in Claude Code (skill from another mg)"
    );
    assert_eq!(skill["fix"], "mg skill setup");
}

#[test]
fn another_mg_first_on_path_warns_with_the_folder_to_put_ahead() {
    let env = env();
    let dir = elsewhere();
    let other = dir.path().join("bin");
    std::fs::create_dir(&other).unwrap();
    std::fs::write(other.join("mg"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(
        other.join("mg"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    let path = format!("{}:/usr/bin:/bin", other.display());
    let run = env.mg_with_env(
        &["--json", "doctor"],
        &[("PATH", &path), ("SHELL", "/bin/zsh")],
    );
    let path_check = check(&run.json(), "path").clone();
    assert_eq!(path_check["status"], "warn");
    let mg = Path::new(env!("CARGO_BIN_EXE_mg"));
    let fix = path_check["fix"].as_str().unwrap();
    let export = format!(
        "echo 'export PATH=\"{}:$PATH\"' >> ",
        mg.parent().unwrap().display()
    );
    assert!(
        fix.starts_with(&export) && fix.ends_with(".zprofile"),
        "{fix}"
    );
}

#[test]
fn a_project_clone_warns_until_init_points_its_remote_at_the_hub() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["project", "add"]).assert_code(0);

    let remote = check(&doctor(&env, repo.root(), 0), "remote").clone();
    assert_eq!(remote["status"], "warn");
    assert_eq!(remote["fix"], "mg init");

    env.mg_at(repo.root(), &["init"]).assert_code(0);
    let doctor = doctor(&env, repo.root(), 0);
    assert_eq!(
        check(&doctor, "remote"),
        &json!({
            "name": "remote",
            "status": "ok",
            "message": "mergegate pushes to project app",
        })
    );
    assert_eq!(statuses(&doctor).last().unwrap().0, "remote");
}

#[test]
fn a_lost_staging_repo_fails_until_its_fix_runs() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["project", "add"]).assert_code(0);
    std::fs::remove_dir_all(env.home().join("projects/app/staging.git")).unwrap();

    let dir = elsewhere();
    let doctor = self::doctor(&env, dir.path(), 1);
    assert_eq!(
        statuses(&doctor),
        pairs(&[
            ("hub", "ok"),
            ("ui", "ok"),
            ("git", "ok"),
            ("project app", "fail"),
            ("languages", "ok"),
            ("skill", "ok"),
            ("path", "ok"),
        ])
    );
    let fix = doctor["checks"][3]["fix"].as_str().unwrap();
    assert_eq!(fix, "mg project set app target main");

    let args: Vec<&str> = fix.split(' ').skip(1).collect();
    env.mg(&args).assert_code(0);
    let doctor = self::doctor(&env, dir.path(), 0);
    assert_eq!(doctor["ok"], true);
}
