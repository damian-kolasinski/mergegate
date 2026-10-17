//! Early conflict detection: the hub test-merges open items as their
//! target moves and sends a conflict to the agent before the operator gets
//! to the item. Open items aren't merged into each other: only the target
//! they merge into counts.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use testkit::{Env, TestRepo};

/// The target polled often, so a commit outside the hub is seen quickly.
fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(vec![("MG_TARGET_POLL_MS".into(), "200".into())])
}

/// A project with a submitted item per `(branch, files)`, in order, so the
/// first is the oldest. The checkout is left on `main`.
fn submitted(items: &[(&str, &[(&str, &str)])]) -> (Env, TestRepo) {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    for (branch, files) in items {
        repo.git(["switch", "--quiet", "-c", branch, "main"]);
        repo.commit_files(branch, files);
        env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    }
    repo.checkout("main");
    (env, repo)
}

fn inbox(env: &Env) -> Value {
    env.mg(&["--json", "inbox"]).assert_code(0).json()
}

/// The inbox once `done` holds of it, which the watch gets to on its own.
fn inbox_once(env: &Env, done: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let inbox = inbox(env);
        if done(&inbox) {
            return inbox;
        }
        assert!(Instant::now() < deadline, "never came: {inbox:#}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Every event the hub logged, as `(item, type)`.
fn log(env: &Env) -> Vec<(String, String)> {
    let log = env
        .mg(&["--json", "events", "--since", "0"])
        .assert_code(0)
        .json();
    log["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["item"].as_str().unwrap().to_owned(),
                e["type"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn ids(group: &Value) -> Vec<&str> {
    group
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect()
}

#[test]
fn f9_a_target_moved_outside_the_hub_sends_the_conflict_before_review() {
    let (env, repo) = submitted(&[("login", &[("src/login.rs", "fn login() {}\n")])]);
    repo.commit_files("Login too", &[("src/login.rs", "fn login(x: u8) {}\n")]);
    let target = repo.rev("main");

    // The agent hears of it without the operator doing anything.
    let feedback = env.mg(&["--json", "wait", "app#1"]).assert_code(0).json();
    assert_eq!(feedback["state"], "changes_requested");
    assert_eq!(
        feedback["events"],
        json!([{
            "cursor": 1,
            "item": "app#1",
            "name": "app#1",
            "revision": 1,
            "source": "operator",
            "type": "merge.conflict",
            "target_commit": target,
            "paths": ["src/login.rs"],
            "commands": ["git fetch mergegate", "git rebase mergegate/main"],
        }])
    );
    let conflict = json!({
        "kind": "conflict",
        "target_commit": target,
        "paths": ["src/login.rs"],
    });
    let waiting = inbox(&env);
    assert_eq!(ids(&waiting["with_agents"]), ["app#1"]);
    assert_eq!(waiting["with_agents"][0]["blocked"], conflict);

    // Once per revision: the target moving on again tells nobody.
    repo.commit_files("Login again", &[("src/login.rs", "fn login(y: u8) {}\n")]);
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(
        log(&env),
        [("app#1".to_owned(), "merge.conflict".to_owned())]
    );

    // A clean rebase is back with the operator, and stays there. Rebased
    // onto where the target is, which the watch moves staging up to.
    let main = repo.rev("main");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !repo
        .git(["ls-remote", "mergegate", "refs/heads/main"])
        .starts_with(&main)
    {
        assert!(Instant::now() < deadline, "staging never followed main");
        std::thread::sleep(Duration::from_millis(50));
    }
    repo.checkout("login");
    repo.git(["fetch", "--quiet", "mergegate"]);
    repo.git(["rebase", "--quiet", "-X", "theirs", "mergegate/main"]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    std::thread::sleep(Duration::from_millis(800));
    let back = inbox(&env);
    assert_eq!(ids(&back["needs_you"]), ["app#1"]);
    assert_eq!(back["needs_you"][0]["blocked"], Value::Null);
    assert_eq!(log(&env).len(), 1);
}

#[test]
fn f9_items_conflicting_only_with_each_other_show_nothing_until_one_lands() {
    let (env, repo) = submitted(&[
        ("greet", &[("README", "hello\n")]),
        ("wave", &[("README", "hey\n")]),
    ]);
    std::thread::sleep(Duration::from_millis(800));
    let both = inbox(&env);
    assert_eq!(ids(&both["needs_you"]), ["app#1", "app#2"]);
    for item in both["needs_you"].as_array().unwrap() {
        assert_eq!(item["target"], "main");
        assert_eq!(item["blocked"], Value::Null);
    }
    assert!(log(&env).is_empty());

    // Landing one makes the other conflict with the target.
    repo.git(["merge", "--quiet", "--ff-only", "greet"]);
    let found = inbox_once(&env, |inbox| inbox["with_agents"] != json!([]));
    assert_eq!(ids(&found["needs_you"]), ["app#1"]);
    assert_eq!(ids(&found["with_agents"]), ["app#2"]);
    assert_eq!(
        found["with_agents"][0]["blocked"]["paths"],
        json!(["README"])
    );
    let text = env.mg(&["inbox"]).assert_code(0).stdout;
    let short = &repo.rev("main")[..12];
    assert!(
        text.contains(&format!("conflicts with main at {short}: README")),
        "{text}"
    );
}
