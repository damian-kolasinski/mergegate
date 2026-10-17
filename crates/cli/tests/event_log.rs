//! Following the hub: `mg events` reads every item's events for
//! the operator's agent, and `--follow` streams them as they come, without
//! taking them from the items' own agents.

use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::Value;
use testkit::{Env, TestRepo};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

/// `app#1` and `app#2`, each with one revision.
fn submitted() -> (Env, TestRepo) {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    for branch in ["one", "two"] {
        repo.git(["switch", "--quiet", "-c", branch, "main"]);
        repo.commit_files(branch, &[(&format!("src/{branch}.rs"), "fn f() {}\n")]);
        env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    }
    (env, repo)
}

#[test]
fn reads_every_items_events_after_a_cursor() {
    let (env, _repo) = submitted();
    let start = env.mg(&["--json", "events"]).assert_code(0).json();
    assert_eq!(start["events"], Value::Array(Vec::new()));
    let cursor = start["cursor"].as_u64().unwrap();

    env.mg(&["abandon", "app#2", "--reason", "Duplicate."])
        .assert_code(0);
    env.mg(&["abandon", "app#1"]).assert_code(0);
    let since = cursor.to_string();
    let log = env
        .mg(&["--json", "events", "--since", &since])
        .assert_code(0)
        .json();
    let events = log["events"].as_array().unwrap();
    let read: Vec<(&str, &str)> = events
        .iter()
        .map(|e| (e["item"].as_str().unwrap(), e["type"].as_str().unwrap()))
        .collect();
    assert_eq!(
        read,
        [("app#2", "item.abandoned"), ("app#1", "item.abandoned")]
    );
    assert_eq!(events[0]["reason"], "Duplicate.");
    assert_eq!(log["cursor"], events[1]["cursor"]);

    // The items' agents still get them.
    let feedback = env
        .mg(&["--json", "feedback", "app#2"])
        .assert_code(0)
        .json();
    assert_eq!(feedback["events"].as_array().unwrap().len(), 1);
}

#[test]
fn follow_streams_events_as_they_come_one_json_object_per_line() {
    let (env, _repo) = submitted();
    // From a cursor rather than from now, so an event logged before the
    // stream's first read is still printed.
    let start = env.mg(&["--json", "events"]).assert_code(0).json();
    let since = start["cursor"].to_string();
    let mut follow = env.spawn(&["--json", "events", "--follow", "--since", &since]);
    let stdout = follow.stdout.take().unwrap();
    let (lines, read) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if lines.send(line.unwrap()).is_err() {
                return;
            }
        }
    });
    let next = || {
        read.recv_timeout(Duration::from_secs(10))
            .expect("an event line")
    };

    env.mg(&["abandon", "app#1", "--reason", "First."])
        .assert_code(0);
    let first: Value = serde_json::from_str(&next()).unwrap();
    assert_eq!(first["item"], "app#1");
    assert_eq!(first["type"], "item.abandoned");
    assert_eq!(first["reason"], "First.");

    env.mg(&["abandon", "app#2"]).assert_code(0);
    let second: Value = serde_json::from_str(&next()).unwrap();
    assert_eq!(second["item"], "app#2");
    assert!(second["cursor"].as_u64() > first["cursor"].as_u64());

    follow.kill().unwrap();
    follow.wait().unwrap();
}
