//! Prioritizing the queue: the authoring agent passes a priority and
//! note from its brief on the item's first submit, the operator's agent
//! changes them with `mg prioritize`, and the inbox orders every group by
//! them.

use serde_json::{Value, json};
use testkit::{Env, TestRepo};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

/// A project with a submitted item per `(branch, flags)`, in order, so the
/// first is the oldest.
fn submitted(items: &[(&str, &[&str])]) -> (Env, TestRepo) {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    for (branch, flags) in items {
        repo.git(["switch", "--quiet", "-c", branch, "main"]);
        repo.commit_files(branch, &[(&format!("src/{branch}.rs"), "fn f() {}\n")]);
        let mut args = vec!["--json", "submit"];
        args.extend_from_slice(flags);
        env.mg_at(repo.root(), &args).assert_code(0);
    }
    (env, repo)
}

/// The inbox's waiting items as `(id, priority, note)`.
fn needs_you(env: &Env) -> Vec<(String, Value, Value)> {
    let inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    inbox["needs_you"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["id"].as_str().unwrap().to_owned(),
                i["priority"].clone(),
                i["note"].clone(),
            )
        })
        .collect()
}

fn ids(env: &Env) -> Vec<String> {
    needs_you(env).into_iter().map(|(id, ..)| id).collect()
}

#[test]
fn f10_the_inbox_orders_by_priority_then_age() {
    let (env, _repo) = submitted(&[
        ("plain", &[]),
        (
            "release",
            &["--priority", "p1", "--note", "blocks the release"],
        ),
        ("hotfix", &["--priority", "p0"]),
    ]);
    assert_eq!(
        needs_you(&env),
        [
            ("app#3".into(), json!("p0"), Value::Null),
            ("app#2".into(), json!("p1"), json!("blocks the release")),
            ("app#1".into(), Value::Null, Value::Null),
        ]
    );
    let text = env.mg(&["inbox"]).assert_code(0).stdout;
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "needs the operator (3)");
    assert!(lines[1].starts_with("  app#3 p0 r1  hotfix"), "{text}");
    assert!(lines[2].starts_with("  app#2 p1 r1  release"), "{text}");
    assert_eq!(lines[3], "      blocks the release");
    assert!(lines[4].starts_with("  app#1 r1  plain"), "{text}");

    // An equal priority keeps the older item first.
    let run = env.mg(&[
        "prioritize",
        "app#1",
        "--priority",
        "p0",
        "--note",
        "the demo",
    ]);
    assert_eq!(run.assert_code(0).stdout.trim(), "app#1 is p0: the demo");
    assert_eq!(ids(&env), ["app#1", "app#3", "app#2"]);

    // Without --note the old note goes with the old priority.
    let item = env
        .mg(&["--json", "prioritize", "app#2", "--priority", "none"])
        .assert_code(0)
        .json();
    assert_eq!(
        (&item["priority"], &item["note"]),
        (&Value::Null, &Value::Null)
    );
    assert_eq!(ids(&env), ["app#1", "app#3", "app#2"]);
}

#[test]
fn f10_a_later_submit_keeps_the_operators_priority() {
    let (env, repo) = submitted(&[("release", &["--priority", "p2", "--note", "brief"])]);
    env.mg(&["prioritize", "app#1", "--priority", "p0"])
        .assert_code(0);

    // The agent repeats its brief on the next revision.
    repo.commit_files("More", &[("src/more.rs", "fn g() {}\n")]);
    let submitted = env
        .mg_at(
            repo.root(),
            &["--json", "submit", "--priority", "p2", "--note", "brief"],
        )
        .assert_code(0)
        .json();
    assert_eq!(submitted["item"]["revision"]["number"], 2);
    assert_eq!(submitted["item"]["priority"], "p0");
    assert_eq!(submitted["item"]["note"], Value::Null);
    assert_eq!(
        submitted["warnings"],
        json!([
            "--priority and --note set only a new item's; app#1 keeps p0, which mg prioritize changes"
        ])
    );
}

#[test]
fn only_an_open_item_is_prioritized() {
    let (env, _repo) = submitted(&[("gone", &[])]);
    env.mg(&["abandon", "app#1"]).assert_code(0);
    let error = env
        .mg(&["--json", "prioritize", "app#1", "--priority", "p0"])
        .assert_code(1)
        .json();
    assert_eq!(error["error"]["code"], "item_closed");
    let error = env
        .mg(&["--json", "prioritize", "app#9", "--priority", "p0"])
        .assert_code(1)
        .json();
    assert_eq!(error["error"]["code"], "item_not_found");
    env.mg(&["prioritize", "app#1", "--priority", "p9"])
        .assert_code(64);
}

#[test]
fn submit_takes_no_priority_of_none() {
    let (env, repo) = submitted(&[("first", &[])]);
    // Read as clearing the priority, which only mg prioritize does.
    repo.commit_files("More", &[("src/more.rs", "fn g() {}\n")]);
    env.mg_at(repo.root(), &["submit", "--priority", "none"])
        .assert_code(64);
}
