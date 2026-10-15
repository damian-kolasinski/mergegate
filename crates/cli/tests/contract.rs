//! The CLI contract agents rely on: exit codes, `--json`, `MG_JSON`, errors
//! that name the next command, and the cold-start and inbox budgets.

use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use testkit::{Env, Reply, TestRepo, http_reply};

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
    assert_eq!(error["hint"], "mg hub --help");

    let run = env.mg(&["--json", "hub"]).assert_code(64);
    let error = &run.json()["error"];
    assert_eq!(error["code"], "usage");
    assert_eq!(error["message"], "a subcommand is required");

    env.mg(&[]).assert_code(64);
}

#[test]
fn json_usage_errors_name_what_is_missing() {
    let env = env();
    let run = env.mg(&["--json", "attach", "app#1"]).assert_code(64);
    let error = &run.json()["error"];
    assert_eq!(
        error["message"],
        "the following required arguments were not provided: <FILES>..."
    );
    assert_eq!(error["hint"], "mg attach --help");

    let run = env.mg(&["--json", "connect"]).assert_code(64);
    let error = &run.json()["error"];
    assert_eq!(
        error["message"],
        "the following required arguments were not provided: <HUB>"
    );
    assert_eq!(error["hint"], "mg connect --help");

    let run = env.mg(&["--json", "lang", "add"]).assert_code(64);
    let error = &run.json()["error"];
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .ends_with("provided: <NAMES|--from <PATH>|--detect [<PROJECT>]>"),
        "{run:?}"
    );
    assert_eq!(error["hint"], "mg lang add --help");

    let run = env.mg(&["--json", "inbox", "--bogus"]).assert_code(64);
    let error = &run.json()["error"];
    assert_eq!(error["message"], "unexpected argument '--bogus' found");
    assert_eq!(error["hint"], "mg inbox --help");
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

/// Budget: `mg inbox --json` under 30 ms (tech-stack §9), over a hub with 20
/// open items. Run and enforced like `cold_start_budget`.
#[test]
#[ignore]
fn inbox_budget() {
    const BUDGET: Duration = Duration::from_millis(30);
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    for n in 0..20 {
        let branch = format!("work-{n}");
        repo.checkout("main");
        repo.git(["switch", "--quiet", "-c", &branch]);
        repo.commit_files(&branch, &[(&format!("{branch}.txt"), "a\nb\n")]);
        env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    }
    let mut samples: Vec<Duration> = (0..30)
        .map(|_| {
            let started = Instant::now();
            env.mg(&["--json", "inbox"]).assert_code(0);
            started.elapsed()
        })
        .collect();
    samples.sort();
    let median = samples[samples.len() / 2];
    println!("mg inbox --json: median {median:?}, budget {BUDGET:?}");
    if std::env::var_os("MG_ENFORCE_BUDGETS").is_some_and(|v| v == "1") {
        assert!(median < BUDGET, "median {median:?} over budget {BUDGET:?}");
    }
}

/// Budget: opening an item under 50 ms (tech-stack §9), comments placed
/// included. `app#1`'s 20 files carry 60 comments from r1, placed onto an r2
/// rebased onto a moved base, so each file's two blobs are read from git and
/// diffed; `app#2` is the same item without comments, and the difference is
/// what placing costs. Run and enforced like `cold_start_budget`.
#[test]
#[ignore]
fn open_item_budget() {
    const BUDGET: Duration = Duration::from_millis(50);
    const FILES: usize = 20;
    const LINES: usize = 300;
    // The lines r1 changes and the operator comments on; r2 changes the
    // middle one again.
    const COMMENTED: [usize; 3] = [50, 150, 250];
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    let path = |file: usize| format!("src/file_{file}.rs");
    let commit = |message: &str, revision: u32, upstream: bool| {
        let files: Vec<(String, String)> = (0..FILES)
            .map(|file| {
                let mut lines: Vec<String> = (1..=LINES)
                    .map(|n| format!("    let line_{n} = {file};"))
                    .collect();
                if revision >= 1 {
                    for n in COMMENTED {
                        lines[n - 1] = format!("    let changed_{n} = {file};");
                    }
                }
                if revision >= 2 {
                    lines[COMMENTED[1] - 1] = format!("    let again = {file};");
                }
                if upstream {
                    lines.insert(0, "// Upstream.".into());
                }
                (path(file), lines.join("\n") + "\n")
            })
            .collect();
        let files: Vec<(&str, &str)> = files
            .iter()
            .map(|(path, source)| (path.as_str(), source.as_str()))
            .collect();
        repo.commit_files(message, &files);
    };
    commit("Base", 0, false);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    let branches = ["commented", "plain"];
    for branch in branches {
        repo.checkout("main");
        repo.git(["switch", "--quiet", "-c", branch]);
        commit("Change", 1, false);
        env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    }
    for file in 0..FILES {
        for line in COMMENTED {
            let draft = json!({
                "revision": 1, "path": path(file), "side": "new",
                "start_line": line, "line": line, "body": "Why this?",
            });
            let reply = send(&env, "POST", "/api/items/app%231/drafts", &draft);
            assert_eq!(reply.status, 201, "{}", reply.body);
        }
    }
    for item in ["app%231", "app%232"] {
        let review = json!({ "revision": 1, "verdict": "changes_requested", "summary": "Fix it" });
        let reply = send(&env, "POST", &format!("/api/items/{item}/reviews"), &review);
        assert_eq!(reply.status, 201, "{}", reply.body);
    }
    repo.checkout("main");
    commit("Upstream", 0, true);
    for branch in branches {
        repo.checkout(branch);
        repo.git(["reset", "--quiet", "--hard", "main"]);
        commit("Change", 2, true);
        env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    }

    let opened = get(&env, "/api/items/app%231");
    assert_eq!(opened["item"]["revision"]["number"], 2);
    let mut kinds: Vec<&str> = opened["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["placement"]["kind"].as_str().unwrap())
        .collect();
    kinds.sort();
    kinds.dedup();
    assert_eq!(kinds, ["changed", "moved"]);

    let median = |path: &str| {
        // The first reads warm the hub's caches, as an operator's would.
        for _ in 0..5 {
            get(&env, path);
        }
        let mut samples: Vec<Duration> = (0..30)
            .map(|_| {
                let started = Instant::now();
                get(&env, path);
                started.elapsed()
            })
            .collect();
        samples.sort();
        samples[samples.len() / 2]
    };
    let enforce = std::env::var_os("MG_ENFORCE_BUDGETS").is_some_and(|v| v == "1");
    for (what, path) in [("item", ""), ("conversation", "/conversation?onto=2")] {
        let placed = median(&format!("/api/items/app%231{path}"));
        let plain = median(&format!("/api/items/app%232{path}"));
        let placing = placed.saturating_sub(plain);
        println!(
            "{what}: median {placed:?} with 60 comments placed, {plain:?} without; \
             placing {placing:?}, {:?} per file; budget {BUDGET:?}",
            placing / FILES as u32,
        );
        if enforce {
            assert!(placed < BUDGET, "median {placed:?} over budget {BUDGET:?}");
        }
    }
}

fn send(env: &Env, method: &str, path: &str, body: &Value) -> Reply {
    let addr = env.hub_addr();
    let body = body.to_string();
    http_reply(
        &addr,
        &format!(
            "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
             {}: ui\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            api::CLIENT_HEADER,
            body.len(),
        ),
    )
}

fn get(env: &Env, path: &str) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap_or_else(|e| panic!("{e}: {}", reply.body))
}
