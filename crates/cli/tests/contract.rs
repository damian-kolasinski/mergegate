//! The CLI contract agents rely on: exit codes, `--json`, `MG_JSON`, errors
//! that name the next command, and the cold-start budget.

use std::process::Command;
use std::time::{Duration, Instant};

use testkit::Env;

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

#[test]
fn usage_errors_exit_64_not_2() {
    let env = env();
    let run = env.mg(&["hub", "frobnicate"]).assert_code(64);
    assert!(run.stdout.is_empty());
    assert!(run.stderr.contains("unrecognized subcommand"), "{run:?}");

    let run = env.mg(&["--json", "hub", "frobnicate"]).assert_code(64);
    let error = &run.json()["error"];
    assert_eq!(error["code"], "usage");
    assert_eq!(error["hint"], "mg --help");

    let run = env.mg(&["--json", "hub"]).assert_code(64);
    let error = &run.json()["error"];
    assert_eq!(error["code"], "usage");
    assert_eq!(error["message"], "a subcommand is required");

    env.mg(&[]).assert_code(64);
}

#[test]
fn help_and_version_exit_0() {
    let env = env();
    let help = env.mg(&["--help"]).assert_code(0);
    assert!(help.stdout.contains("Exit codes:"), "{help:?}");
    let version = env.mg(&["--version"]).assert_code(0);
    assert_eq!(
        version.stdout.trim(),
        format!("mg {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn mg_json_env_turns_on_json_output() {
    let env = env();
    let run = env
        .mg_with_env(&["hub", "status"], &[("MG_JSON", "1")])
        .assert_code(0);
    assert_eq!(run.json()["running"], false);
}

#[test]
fn errors_name_the_fix() {
    let env = env();
    let run = env
        .mg_with_env(&["hub", "status"], &[("MERGEGATE_HOME", "relative/dir")])
        .assert_code(1);
    assert!(run.stdout.is_empty());
    assert_eq!(
        run.stderr.trim(),
        "error: MERGEGATE_HOME must be an absolute path, got \"relative/dir\"\n\
         hint: set MERGEGATE_HOME to an absolute path"
    );
}

/// Budget: `mg --version` cold start under 10 ms (tech-stack §9). Meaningful
/// only for release builds, so CI runs it with
/// `cargo test --release -p mg --test contract -- --ignored`; it fails over
/// budget when `MG_ENFORCE_BUDGETS=1` and otherwise only reports.
#[test]
#[ignore]
fn cold_start_budget() {
    const BUDGET: Duration = Duration::from_millis(10);
    let mut samples: Vec<Duration> = (0..50)
        .map(|_| {
            let started = Instant::now();
            let status = Command::new(env!("CARGO_BIN_EXE_mg"))
                .arg("--version")
                .output()
                .unwrap()
                .status;
            assert!(status.success());
            started.elapsed()
        })
        .collect();
    samples.sort();
    let median = samples[samples.len() / 2];
    println!("mg --version: median {median:?}, budget {BUDGET:?}");
    if std::env::var_os("MG_ENFORCE_BUDGETS").is_some_and(|v| v == "1") {
        assert!(median < BUDGET, "median {median:?} over budget {BUDGET:?}");
    }
}
