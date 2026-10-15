//! Performance budgets (tech-stack §9) for the paths agents and the operator
//! wait on: an agent's submit, its wake on feedback and its read of it, and
//! the operator's comment, viewed mark, review and merge. Meaningful only for
//! release builds, so CI runs them with
//! `cargo test --release -p mg --test budgets -- --ignored`; each fails over
//! budget when `MG_ENFORCE_BUDGETS=1` and otherwise only reports.

use std::io::Read;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use testkit::{Env, Reply, TestRepo, http_reply};

const FILES: usize = 20;
const LINES: usize = 300;

/// Budget: `mg submit` of a 20-file, 300-line-a-file change under 150 ms,
/// pushing included, both the submit that opens an item and the one that
/// adds a revision (its interdiffs and viewed carry-over included).
#[test]
#[ignore]
fn submit_budget() {
    const BUDGET: Duration = Duration::from_millis(150);
    let (env, repo) = project();
    let mut opened = Vec::new();
    let mut revised = Vec::new();
    for n in 0..10 {
        let branch = format!("work-{n}");
        repo.checkout("main");
        repo.git(["switch", "--quiet", "-c", &branch]);
        change(&repo, 1);
        opened.push(timed(|| {
            env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
        }));
        change(&repo, 2);
        revised.push(timed(|| {
            env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
        }));
    }
    report("mg submit, new item", opened, BUDGET);
    report("mg submit, new revision", revised, BUDGET);
}

/// Budget: an agent blocked in `mg wait` exits within 50 ms of the operator
/// submitting a review, the review's request included; and `mg feedback`
/// reads a review with 60 comments in under 30 ms.
#[test]
#[ignore]
fn feedback_budget() {
    const WAKE: Duration = Duration::from_millis(50);
    const READ: Duration = Duration::from_millis(30);
    let (env, repo) = project();
    repo.git(["switch", "--quiet", "-c", "work"]);
    change(&repo, 1);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);

    let mut woke = Vec::new();
    for n in 0..10 {
        let mut wait = env.spawn(&["--json", "wait", "app#1", "--timeout", "30"]);
        // Long enough for the wait's request to reach the hub and block.
        std::thread::sleep(Duration::from_millis(300));
        let review = json!({ "revision": 1, "verdict": "comment", "summary": format!("Note {n}") });
        let started = Instant::now();
        let reply = send(&env, "POST", "/api/items/app%231/reviews", &review);
        assert_eq!(reply.status, 201, "{}", reply.body);
        let status = wait.wait().unwrap();
        woke.push(started.elapsed());
        assert!(status.success(), "mg wait exited {status}");
        let mut out = String::new();
        wait.stdout
            .take()
            .unwrap()
            .read_to_string(&mut out)
            .unwrap();
        assert!(out.contains(&format!("Note {n}")), "{out}");
    }
    report("mg wait, woken by a review", woke, WAKE);

    for file in 0..FILES {
        for line in COMMENTED {
            draft(&env, "app%231", file, line);
        }
    }
    let review = json!({ "revision": 1, "verdict": "changes_requested", "summary": "Fix it" });
    let reply = send(&env, "POST", "/api/items/app%231/reviews", &review);
    assert_eq!(reply.status, 201, "{}", reply.body);
    let read = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    let cursor = read["cursor"].as_u64().unwrap();
    // Replaying from before the review reads it whole every time.
    let since = (cursor - 1).to_string();
    let reads = samples(30, || {
        env.mg(&["--json", "feedback", "app#1", "--since", &since])
            .assert_code(0);
    });
    report("mg feedback, 60 comments", reads, READ);
}

/// Budget: the operator's actions on an open item answer under 16 ms, so
/// the UI never shows them pending: writing a comment, marking a file
/// viewed, and submitting a review of 60 comments. Approving merges, which
/// runs git, so it gets 150 ms.
#[test]
#[ignore]
fn review_actions_budget() {
    const ACTION: Duration = Duration::from_millis(16);
    const MERGE: Duration = Duration::from_millis(150);
    let (env, repo) = project();
    let items = 10;
    for n in 0..items {
        let branch = format!("work-{n}");
        repo.checkout("main");
        repo.git(["switch", "--quiet", "-c", &branch]);
        // Each item its own file, so they merge one after another cleanly.
        let file = format!("src/new_{n}.rs");
        let source = (1..=LINES)
            .map(|l| format!("let x_{l} = {n};\n"))
            .collect::<String>();
        repo.commit_files("Add", &[(&file, &source)]);
        env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    }
    repo.checkout("main");
    repo.git(["switch", "--quiet", "-c", "big"]);
    change(&repo, 1);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    let big = format!("app%23{}", items + 1);

    // Several reviews, each of 60 fresh comments, so one slow sample on a
    // shared runner doesn't decide the budget.
    let mut comments = Vec::new();
    let mut submitted = Vec::new();
    for _ in 0..5 {
        for file in 0..FILES {
            for line in COMMENTED {
                comments.push(timed(|| draft(&env, &big, file, line)));
            }
        }
        let review = json!({ "revision": 1, "verdict": "comment", "summary": "Note" });
        submitted.push(timed(|| {
            let reply = send(&env, "POST", &format!("/api/items/{big}/reviews"), &review);
            assert_eq!(reply.status, 201, "{}", reply.body);
        }));
    }
    let viewed = (0..FILES)
        .map(|file| {
            let mark = json!({ "revision": 1, "path": path(file), "viewed": true });
            timed(|| {
                let reply = send(&env, "PUT", &format!("/api/items/{big}/viewed"), &mark);
                assert!(reply.status < 300, "{}", reply.body);
            })
        })
        .collect();
    let merged = (1..=items)
        .map(|n| {
            let approve = json!({ "revision": 1, "summary": "" });
            timed(|| {
                let reply = send(
                    &env,
                    "POST",
                    &format!("/api/items/app%23{n}/approve"),
                    &approve,
                );
                assert_eq!(reply.status, 200, "{}", reply.body);
                let merge: Value = serde_json::from_str(&reply.body).unwrap();
                assert_eq!(merge["outcome"]["kind"], "merged", "{merge}");
            })
        })
        .collect();
    report("write a comment", comments, ACTION);
    report("mark a file viewed", viewed, ACTION);
    report("submit a review of 60 comments", submitted, ACTION);
    report("approve and merge", merged, MERGE);
}

/// The lines each revision changes and comments land on.
const COMMENTED: [usize; 3] = [50, 150, 250];

/// A hub with project `app`, whose `main` holds `FILES` files of `LINES`
/// lines.
fn project() -> (Env, TestRepo) {
    let env = Env::new(env!("CARGO_BIN_EXE_mg"));
    env.start_hub();
    let repo = TestRepo::new("app");
    change(&repo, 0);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    (env, repo)
}

fn path(file: usize) -> String {
    format!("src/file_{file}.rs")
}

/// Commits every file as `revision` has it: 0 is the base, 1 changes the
/// `COMMENTED` lines, 2 the middle one again.
fn change(repo: &TestRepo, revision: u32) {
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
            (path(file), lines.join("\n") + "\n")
        })
        .collect();
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect();
    repo.commit_files(&format!("Revision {revision}"), &files);
}

fn draft(env: &Env, item: &str, file: usize, line: usize) {
    let draft = json!({
        "revision": 1, "path": path(file), "side": "new",
        "start_line": line, "line": line, "body": "Why this?",
    });
    let reply = send(env, "POST", &format!("/api/items/{item}/drafts"), &draft);
    assert_eq!(reply.status, 201, "{}", reply.body);
}

fn timed(run: impl FnOnce()) -> Duration {
    let started = Instant::now();
    run();
    started.elapsed()
}

fn samples(count: usize, mut run: impl FnMut()) -> Vec<Duration> {
    (0..count).map(|_| timed(&mut run)).collect()
}

fn report(what: &str, mut samples: Vec<Duration>, budget: Duration) {
    samples.sort();
    let median = samples[samples.len() / 2];
    let max = samples[samples.len() - 1];
    println!("{what}: median {median:?}, max {max:?}, budget {budget:?}");
    if std::env::var_os("MG_ENFORCE_BUDGETS").is_some_and(|v| v == "1") {
        assert!(
            median < budget,
            "{what}: median {median:?} over budget {budget:?}"
        );
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
