//! Review time: the UI reports how long the operator was active on an item,
//! and the item's closing event carries the totals, per turn and per
//! revision, for its agent's telemetry, as both the item's feedback and the
//! hub's event log read it.
//!
//! Neither the operator nor the agent sees both halves, so each lifecycle an
//! item can go through has its own test here, and every closing event is
//! checked for the invariants the stats keep whatever the lifecycle.

use serde_json::{Value, json};
use testkit::{Env, FakeGithub, Reply, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(vec![("MG_TARGET_WATCH".into(), "off".into())])
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

fn json_of(reply: &Reply) -> Value {
    serde_json::from_str(&reply.body).unwrap_or_else(|e| panic!("{e}: {}", reply.body))
}

fn path(item: &str) -> String {
    format!("/api/items/{}", item.replace('#', "%23"))
}

/// A project on `main` with one commit, its hub running.
fn project(env: &Env) -> TestRepo {
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo
}

/// `app#1` with one revision, waiting for the operator.
fn submitted() -> (Env, TestRepo) {
    let env = env();
    let repo = project(&env);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    (env, repo)
}

/// The agent's next revision.
fn revise(env: &Env, repo: &TestRepo) {
    repo.commit_files("Add logout", &[("src/logout.rs", "fn logout() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
}

/// What the UI sends after measuring `ms` on `revision` of `item`.
fn spend(env: &Env, item: &str, revision: u32, ms: u64) -> Reply {
    let body = json!({ "revision": revision, "ms": ms });
    send(env, "POST", &format!("{}/review-time", path(item)), &body)
}

fn spent(env: &Env, item: &str, revision: u32, ms: u64) {
    let reply = spend(env, item, revision, ms);
    assert_eq!(reply.status, 204, "{}", reply.body);
}

fn refused(env: &Env, item: &str, revision: u32, code: &str) {
    let reply = spend(env, item, revision, 1_000);
    assert_eq!(reply.status, 409, "{}", reply.body);
    assert_eq!(json_of(&reply)["error"]["code"], code, "{}", reply.body);
}

fn review(env: &Env, item: &str, revision: u32, verdict: &str) {
    let body = json!({ "revision": revision, "verdict": verdict, "summary": "Fix it." });
    let reply = send(env, "POST", &format!("{}/reviews", path(item)), &body);
    assert_eq!(reply.status, 201, "{}", reply.body);
}

/// The operator approves `revision`; the reply's outcome.
fn approve(env: &Env, item: &str, revision: u32) -> Value {
    let body = json!({ "revision": revision, "summary": "" });
    let reply = send(env, "POST", &format!("{}/approve", path(item)), &body);
    assert_eq!(reply.status, 200, "{}", reply.body);
    json_of(&reply)["outcome"].clone()
}

/// Dirties the operator's checkout of `main`, so a local merge is held.
fn hold_merges(repo: &TestRepo) {
    repo.checkout("main");
    std::fs::write(repo.root().join("README"), "edited\n").unwrap();
}

fn release_merges(repo: &TestRepo) {
    std::fs::write(repo.root().join("README"), "title\n").unwrap();
}

/// `item`'s `kind` event, which must read the same through the item's
/// feedback and the hub's event log.
fn closing_event(env: &Env, item: &str, kind: &str) -> Value {
    let feedback = env.mg(&["--json", "feedback", item]).json();
    let event = feedback["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == kind)
        .unwrap_or_else(|| panic!("no {kind} in {feedback}"))
        .clone();
    let log = env
        .mg(&["--json", "events", "--since", "0"])
        .assert_code(0)
        .json();
    let logged = log["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == kind && e["item"] == item)
        .unwrap_or_else(|| panic!("no {kind} for {item} in {log}"));
    assert_eq!(logged.get("stats"), event.get("stats"), "{logged}");
    event
}

/// The stats on `item`'s `kind` event, checked for what holds in every
/// lifecycle.
fn closing(env: &Env, item: &str, kind: &str) -> Value {
    let event = closing_event(env, item, kind);
    let stats = event["stats"].clone();
    assert!(stats.is_object(), "no stats on {event}");
    invariants(&stats);
    stats
}

fn invariants(stats: &Value) {
    let ms = |entries: &Value| -> u64 {
        entries
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["review_ms"].as_u64().unwrap())
            .sum()
    };
    let total = stats["review_ms"].as_u64().unwrap();
    assert_eq!(ms(&stats["turns"]), total, "turns add up: {stats}");
    assert_eq!(ms(&stats["revisions"]), total, "revisions add up: {stats}");
    let opened = stats["opened_at"].as_str().unwrap();
    let closed = stats["closed_at"].as_str().unwrap();
    assert!(opened <= closed, "{stats}");
    let turns = stats["turns"].as_array().unwrap();
    for (at, turn) in turns.iter().enumerate() {
        assert_eq!(turn["turn"], at + 1, "turns count from 1: {stats}");
        // Only the last can be under way: every other ended with a review.
        let last = at + 1 == turns.len();
        assert_eq!(
            turn["verdict"].is_null(),
            turn["sent_at"].is_null(),
            "{stats}"
        );
        if turn["verdict"].is_null() {
            assert!(last, "only the last turn is under way: {stats}");
            assert!(turn["review_ms"].as_u64().unwrap() > 0, "{stats}");
        } else {
            let sent = turn["sent_at"].as_str().unwrap();
            assert!(opened <= sent && sent <= closed, "{stats}");
        }
    }
    for (at, revision) in stats["revisions"].as_array().unwrap().iter().enumerate() {
        assert_eq!(revision["revision"], at + 1, "{stats}");
    }
}

/// `(turn, revision, verdict, review_ms)` of each turn.
fn turns(stats: &Value) -> Vec<(Value, Value, Value, Value)> {
    stats["turns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["turn"].clone(),
                t["revision"].clone(),
                t["verdict"].clone(),
                t["review_ms"].clone(),
            )
        })
        .collect()
}

fn turn(
    number: u32,
    revision: u32,
    verdict: Option<&str>,
    ms: u64,
) -> (Value, Value, Value, Value) {
    (json!(number), json!(revision), json!(verdict), json!(ms))
}

/// `(revision, review_ms)` of each revision.
fn revisions(stats: &Value) -> Vec<(Value, Value)> {
    stats["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["revision"].clone(), r["review_ms"].clone()))
        .collect()
}

fn revision(number: u32, ms: u64) -> (Value, Value) {
    (json!(number), json!(ms))
}

#[test]
fn a_merge_carries_the_operators_review_time_per_turn_and_revision() {
    let (env, repo) = submitted();
    spent(&env, "app#1", 1, 30_000);
    review(&env, "app#1", 1, "changes_requested");
    revise(&env, &repo);
    spent(&env, "app#1", 2, 12_000);
    spent(&env, "app#1", 2, 3_000);
    assert_eq!(approve(&env, "app#1", 2)["kind"], "merged");

    let stats = closing(&env, "app#1", "item.merged");
    assert_eq!(stats["review_ms"], 45_000, "{stats}");
    assert_eq!(
        turns(&stats),
        [
            turn(1, 1, Some("changes_requested"), 30_000),
            turn(2, 2, Some("approved"), 15_000),
        ],
        "{stats}"
    );
    assert_eq!(
        revisions(&stats),
        [revision(1, 30_000), revision(2, 15_000)],
        "{stats}"
    );
    assert_eq!(stats["revisions"][0]["submitted_at"], stats["opened_at"]);
}

#[test]
fn a_review_answered_with_replies_alone_is_a_turn_on_the_same_revision() {
    let (env, _repo) = submitted();
    let draft = json!({
        "revision": 1, "path": "src/login.rs", "side": "new",
        "start_line": 1, "line": 1, "body": "Check the password.",
    });
    let drafted = send(&env, "POST", "/api/items/app%231/drafts", &draft);
    assert!(drafted.status < 300, "{}", drafted.body);
    spent(&env, "app#1", 1, 10_000);
    review(&env, "app#1", 1, "changes_requested");
    env.mg(&[
        "reply",
        "c_1",
        "--status",
        "question",
        "--body",
        "Empty or missing?",
    ])
    .assert_code(0);
    spent(&env, "app#1", 1, 4_000);
    let answer = send(
        &env,
        "POST",
        "/api/comments/c_1/drafts",
        &json!({ "body": "Both." }),
    );
    assert!(answer.status < 300, "{}", answer.body);
    review(&env, "app#1", 1, "comment");
    spent(&env, "app#1", 1, 2_000);
    approve(&env, "app#1", 1);

    let stats = closing(&env, "app#1", "item.merged");
    assert_eq!(
        turns(&stats),
        [
            turn(1, 1, Some("changes_requested"), 10_000),
            turn(2, 1, Some("comment"), 4_000),
            turn(3, 1, Some("approved"), 2_000),
        ],
        "{stats}"
    );
    assert_eq!(revisions(&stats), [revision(1, 16_000)], "{stats}");
}

#[test]
fn a_revision_nobody_reviewed_starts_no_turn() {
    let (env, repo) = submitted();
    spent(&env, "app#1", 1, 3_000);
    // The agent rebased before the operator sent anything.
    revise(&env, &repo);
    spent(&env, "app#1", 2, 7_000);
    approve(&env, "app#1", 2);

    let stats = closing(&env, "app#1", "item.merged");
    assert_eq!(
        turns(&stats),
        [turn(1, 2, Some("approved"), 10_000)],
        "{stats}"
    );
    assert_eq!(
        revisions(&stats),
        [revision(1, 3_000), revision(2, 7_000)],
        "{stats}"
    );
}

#[test]
fn a_held_merge_takes_no_time_and_its_retry_ends_no_turn() {
    let (env, repo) = submitted();
    spent(&env, "app#1", 1, 5_000);
    hold_merges(&repo);
    assert_eq!(approve(&env, "app#1", 1)["kind"], "held");

    // The approval ended the last turn; time now would start one after it.
    refused(&env, "app#1", 1, "item_merging");
    release_merges(&repo);
    let retried = send(&env, "POST", "/api/items/app%231/merge", &json!({}));
    assert_eq!(
        json_of(&retried)["outcome"]["kind"],
        "merged",
        "{}",
        retried.body
    );

    let stats = closing(&env, "app#1", "item.merged");
    assert_eq!(
        turns(&stats),
        [turn(1, 1, Some("approved"), 5_000)],
        "{stats}"
    );
}

#[test]
fn a_cancelled_merge_puts_the_item_back_in_review_for_another_turn() {
    let (env, repo) = submitted();
    spent(&env, "app#1", 1, 5_000);
    hold_merges(&repo);
    approve(&env, "app#1", 1);
    let cancelled = send(&env, "DELETE", "/api/items/app%231/merge", &json!({}));
    assert_eq!(
        json_of(&cancelled)["state"],
        "needs_review",
        "{}",
        cancelled.body
    );

    spent(&env, "app#1", 1, 2_000);
    release_merges(&repo);
    assert_eq!(approve(&env, "app#1", 1)["kind"], "merged");

    let stats = closing(&env, "app#1", "item.merged");
    assert_eq!(
        turns(&stats),
        [
            turn(1, 1, Some("approved"), 5_000),
            turn(2, 1, Some("approved"), 2_000),
        ],
        "{stats}"
    );
}

#[test]
fn an_approval_carried_to_an_unchanged_rebase_ends_no_turn() {
    let (env, repo) = submitted();
    spent(&env, "app#1", 1, 6_000);
    hold_merges(&repo);
    assert_eq!(approve(&env, "app#1", 1)["kind"], "held");
    release_merges(&repo);
    repo.commit_files("Docs", &[("docs.md", "# Docs\n")]);
    repo.checkout("feature");
    repo.git(["rebase", "--quiet", "main"]);

    // The hub approves r2 on its own, as its diff is r1's.
    let resubmitted = env
        .mg_at(repo.root(), &["--json", "submit"])
        .assert_code(0)
        .json();
    assert_eq!(resubmitted["item"]["state"], "merged", "{resubmitted}");

    let stats = closing(&env, "app#1", "item.merged");
    assert_eq!(
        turns(&stats),
        [turn(1, 1, Some("approved"), 6_000)],
        "{stats}"
    );
    assert_eq!(
        revisions(&stats),
        [revision(1, 6_000), revision(2, 0)],
        "{stats}"
    );
}

#[test]
fn a_close_with_no_review_time_carries_no_stats() {
    let (env, _repo) = submitted();
    approve(&env, "app#1", 1);

    // Zeros would pass for a review that took no time, as on an item closed
    // before the hub measured any.
    let event = closing_event(&env, "app#1", "item.merged");
    assert_eq!(event.get("stats"), None, "{event}");
}

#[test]
fn an_abandon_mid_turn_reports_the_turn_under_way_and_stays_put() {
    let (env, repo) = submitted();
    spent(&env, "app#1", 1, 2_000);
    review(&env, "app#1", 1, "changes_requested");
    revise(&env, &repo);
    spent(&env, "app#1", 2, 5_000);
    env.mg(&["abandon", "app#1", "--reason", "Not needed."])
        .assert_code(0);

    refused(&env, "app#1", 2, "item_closed");
    let stats = closing(&env, "app#1", "item.abandoned");
    assert_eq!(
        turns(&stats),
        [
            turn(1, 1, Some("changes_requested"), 2_000),
            turn(2, 2, None, 5_000),
        ],
        "{stats}"
    );
}

/// `app#1` on a GitHub project, with one revision.
fn on_github() -> (FakeGithub, Env) {
    let github = FakeGithub::start();
    let env = Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(github.env());
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    repo.git(["remote", "add", "origin", "git@github.com:acme/app.git"]);
    github.create_repo("acme/app", repo.root());
    env.mg_at(
        repo.root(),
        &["--json", "project", "add", "--backend", "github"],
    )
    .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "login"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    (github, env)
}

#[test]
fn a_merge_on_github_mid_turn_reports_the_turn_under_way() {
    let (github, env) = on_github();
    spent(&env, "app#1", 1, 9_000);
    review(&env, "app#1", 1, "changes_requested");
    spent(&env, "app#1", 1, 4_000);
    github.merge_on_github("acme/app", 1);
    env.mg(&["--json", "sync", "app#1"]).assert_code(0);

    refused(&env, "app#1", 1, "item_closed");
    let stats = closing(&env, "app#1", "item.merged");
    assert_eq!(
        turns(&stats),
        [
            turn(1, 1, Some("changes_requested"), 9_000),
            turn(2, 1, None, 4_000),
        ],
        "{stats}"
    );
}

/// `app#2` on `profile`, stacked on `app#1` on `login`.
fn stacked() -> (Env, TestRepo) {
    let env = env();
    let repo = project(&env);
    repo.git(["switch", "--quiet", "-c", "login"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "profile"]);
    repo.commit_files("Add profile", &[("src/profile.rs", "fn profile() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit", "--on", "1"])
        .assert_code(0);
    (env, repo)
}

#[test]
fn a_stacked_item_waiting_on_its_parent_takes_no_time() {
    let (env, _repo) = stacked();
    spent(&env, "app#2", 1, 2_000);
    assert_eq!(approve(&env, "app#2", 1)["kind"], "waiting");
    refused(&env, "app#2", 1, "item_merging");
    spent(&env, "app#1", 1, 1_000);
    assert_eq!(approve(&env, "app#1", 1)["kind"], "merged");

    let parent = closing(&env, "app#1", "item.merged");
    assert_eq!(
        turns(&parent),
        [turn(1, 1, Some("approved"), 1_000)],
        "{parent}"
    );
    let child = closing(&env, "app#2", "item.merged");
    assert_eq!(
        turns(&child),
        [turn(1, 1, Some("approved"), 2_000)],
        "{child}"
    );
}

#[test]
fn an_abandoned_parent_reopens_its_waiting_child_for_another_turn() {
    let (env, repo) = stacked();
    spent(&env, "app#2", 1, 2_000);
    assert_eq!(approve(&env, "app#2", 1)["kind"], "waiting");
    spent(&env, "app#1", 1, 1_000);
    env.mg(&["abandon", "app#1"]).assert_code(0);

    // Its approval no longer stands, so it's back in review on r1, then on
    // the r2 that moves it onto the target.
    spent(&env, "app#2", 1, 1_500);
    let notice = env.mg(&["--json", "feedback", "app#2"]).json()["events"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(notice["type"], "parent.abandoned", "{notice}");
    for command in notice["commands"].as_array().unwrap() {
        let args: Vec<_> = command.as_str().unwrap().split(' ').collect();
        repo.git(&args[1..]);
    }
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    spent(&env, "app#2", 2, 1_500);
    assert_eq!(approve(&env, "app#2", 2)["kind"], "merged");

    let parent = closing(&env, "app#1", "item.abandoned");
    assert_eq!(turns(&parent), [turn(1, 1, None, 1_000)], "{parent}");
    let child = closing(&env, "app#2", "item.merged");
    assert_eq!(
        turns(&child),
        [
            turn(1, 1, Some("approved"), 2_000),
            turn(2, 2, Some("approved"), 3_000),
        ],
        "{child}"
    );
    assert_eq!(
        revisions(&child),
        [revision(1, 3_500), revision(2, 1_500)],
        "{child}"
    );
}

#[test]
fn a_conflict_after_the_approval_reopens_review_for_another_turn() {
    let (env, repo) = submitted();
    repo.checkout("main");
    repo.commit_files("Login too", &[("src/login.rs", "fn login(x: u8) {}\n")]);
    repo.checkout("feature");
    spent(&env, "app#1", 1, 5_000);
    assert_eq!(approve(&env, "app#1", 1)["kind"], "conflict");

    // Back with the agent: the operator's time now goes to the next turn.
    spent(&env, "app#1", 1, 2_000);
    repo.git(["fetch", "--quiet", "mergegate"]);
    repo.git(["rebase", "--quiet", "-X", "theirs", "mergegate/main"]);
    let resubmitted = env
        .mg_at(repo.root(), &["--json", "submit"])
        .assert_code(0)
        .json();
    assert_eq!(
        resubmitted["item"]["state"], "needs_review",
        "{resubmitted}"
    );
    spent(&env, "app#1", 2, 3_000);
    assert_eq!(approve(&env, "app#1", 2)["kind"], "merged");

    let stats = closing(&env, "app#1", "item.merged");
    assert_eq!(
        turns(&stats),
        [
            turn(1, 1, Some("approved"), 5_000),
            turn(2, 2, Some("approved"), 5_000),
        ],
        "{stats}"
    );
    assert_eq!(
        revisions(&stats),
        [revision(1, 7_000), revision(2, 3_000)],
        "{stats}"
    );
}

#[test]
fn a_pull_request_closed_on_github_reports_the_turn_under_way() {
    let (github, env) = on_github();
    spent(&env, "app#1", 1, 4_000);
    github.close_on_github("acme/app", 1);
    env.mg(&["--json", "sync", "app#1"]).assert_code(0);

    refused(&env, "app#1", 1, "item_closed");
    let stats = closing(&env, "app#1", "item.abandoned");
    assert_eq!(turns(&stats), [turn(1, 1, None, 4_000)], "{stats}");
}

#[test]
fn an_abandoned_held_merge_ends_on_its_approval() {
    let (env, repo) = submitted();
    spent(&env, "app#1", 1, 5_000);
    hold_merges(&repo);
    assert_eq!(approve(&env, "app#1", 1)["kind"], "held");
    env.mg(&["abandon", "app#1"]).assert_code(0);

    let stats = closing(&env, "app#1", "item.abandoned");
    assert_eq!(
        turns(&stats),
        [turn(1, 1, Some("approved"), 5_000)],
        "{stats}"
    );
}

#[test]
fn an_abandon_with_no_review_time_carries_no_stats() {
    let (env, _repo) = submitted();
    env.mg(&["abandon", "app#1"]).assert_code(0);

    let event = closing_event(&env, "app#1", "item.abandoned");
    assert_eq!(event.get("stats"), None, "{event}");
}

#[test]
fn refuses_time_on_a_revision_the_item_lacks() {
    let (env, _repo) = submitted();

    let refused = spend(&env, "app#1", 2, 1_000);

    assert_eq!(refused.status, 404, "{}", refused.body);
}
