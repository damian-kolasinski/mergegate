//! `mg doctor`: every check, and the command that fixes each failure.

use serde_json::{Value, json};
use testkit::{Env, TestRepo};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
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

#[test]
fn without_a_hub_doctor_fails_and_says_how_to_start_it() {
    let env = env();
    let doctor = env.mg(&["--json", "doctor"]).assert_code(1).json();
    assert_eq!(
        doctor,
        json!({"ok": false, "checks": [{
            "name": "hub",
            "status": "fail",
            "message": "the hub is not running",
            "fix": "mg hub start",
        }]})
    );

    let text = env.mg(&["doctor"]).assert_code(1).stdout;
    assert_eq!(
        text.trim_end(),
        "FAIL  hub  the hub is not running\n           fix: mg hub start"
    );
}

#[test]
fn a_fresh_hub_passes_with_a_warning_until_a_project_is_added() {
    let env = env();
    env.start_hub();
    let doctor = env.mg(&["--json", "doctor"]).assert_code(0).json();
    assert_eq!(doctor["ok"], true);
    assert_eq!(
        statuses(&doctor),
        pairs(&[("hub", "ok"), ("git", "ok"), ("projects", "warn")])
    );
    assert_eq!(doctor["checks"][2]["fix"], "mg project add PATH");
}

#[test]
fn a_lost_staging_repo_fails_until_its_fix_runs() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["project", "add"]).assert_code(0);
    std::fs::remove_dir_all(env.home().join("projects/app/staging.git")).unwrap();

    let doctor = env.mg(&["--json", "doctor"]).assert_code(1).json();
    assert_eq!(
        statuses(&doctor),
        pairs(&[("hub", "ok"), ("git", "ok"), ("project app", "fail")])
    );
    let fix = doctor["checks"][2]["fix"].as_str().unwrap();
    assert_eq!(fix, "mg project set app target main");

    let args: Vec<&str> = fix.split(' ').skip(1).collect();
    env.mg(&args).assert_code(0);
    let doctor = env.mg(&["--json", "doctor"]).assert_code(0).json();
    assert_eq!(doctor["ok"], true);
}
