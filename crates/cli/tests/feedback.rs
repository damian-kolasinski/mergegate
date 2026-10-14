//! The operator's review reaching the agent (F6's last step, F7): the hub
//! logs it, `mg feedback` delivers it once, and `--since` replays it.

use serde_json::{Value, json};
use testkit::{Env, Reply, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn post(env: &Env, path: &str, body: &Value) -> Reply {
    let addr = env.hub_addr();
    let body = body.to_string();
    http_reply(
        &addr,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
             {}: ui\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            api::CLIENT_HEADER,
            body.len(),
        ),
    )
}

fn json_of(reply: &Reply) -> Value {
    serde_json::from_str(&reply.body).unwrap_or_else(|e| panic!("{e}: {}", reply.body))
}

/// `app#1` with one revision, waiting for the operator.
fn submitted() -> (Env, TestRepo) {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Login", &[("src/login.rs", "fn login() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    (env, repo)
}

fn review(env: &Env, verdict: &str, summary: &str) -> Value {
    let reply = post(
        env,
        "/api/items/app%231/reviews",
        &json!({ "revision": 1, "verdict": verdict, "summary": summary }),
    );
    assert_eq!(reply.status, 201, "{}", reply.body);
    json_of(&reply)
}

fn state(env: &Env) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!("GET /api/items/app%231 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    json_of(&reply)["item"]["state"].clone()
}

#[test]
fn f7_feedback_returns_the_review_once() {
    let (env, _repo) = submitted();
    let submitted = review(
        &env,
        "changes_requested",
        "Handle the empty password.\nThanks!",
    );
    assert_eq!(submitted["verdict"], "changes_requested");
    assert_eq!(submitted["revision"], 1);
    assert_eq!(state(&env), "changes_requested");

    let run = env.mg(&["--json", "feedback", "app#1"]).assert_code(0);
    let feedback = run.json();
    assert_eq!(feedback["item"], "app#1");
    assert_eq!(feedback["state"], "changes_requested");
    assert_eq!(feedback["cursor"], 1);
    assert_eq!(
        feedback["events"],
        json!([{
            "cursor": 1,
            "item": "app#1",
            "revision": 1,
            "source": "operator",
            "type": "review.submitted",
            "verdict": "changes_requested",
            "summary": "Handle the empty password.\nThanks!",
        }])
    );

    // Printing acknowledged it, so it isn't delivered again.
    let again = env.mg(&["--json", "feedback", "app#1"]).assert_code(0);
    assert_eq!(again.json()["events"], json!([]));
    let text = env.mg(&["feedback", "app#1"]).assert_code(0);
    assert_eq!(text.stdout.trim(), "no new feedback on app#1");
}

#[test]
fn f7_feedback_prints_the_review_and_how_to_replay_it() {
    let (env, _repo) = submitted();
    review(&env, "changes_requested", "Handle the empty password.");

    let run = env.mg(&["feedback", "app#1"]).assert_code(0);
    assert_eq!(
        run.stdout.trim(),
        "[1] app#1 r1: the operator requested changes\n  Handle the empty password.\n\n\
         cursor 1 (mg feedback --since 1 replays from here)"
    );
}

#[test]
fn f7_since_replays_delivered_events() {
    let (env, _repo) = submitted();
    review(&env, "comment", "First look.");
    review(&env, "changes_requested", "Second look.");
    env.mg(&["--json", "feedback", "app#1"]).assert_code(0);

    let replay = env
        .mg(&["--json", "feedback", "app#1", "--since", "1"])
        .assert_code(0)
        .json();
    let cursors: Vec<_> = replay["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["cursor"].clone())
        .collect();
    assert_eq!(cursors, [json!(2)]);
    assert_eq!(replay["events"][0]["summary"], "Second look.");

    let run = env
        .mg(&["--json", "feedback", "app#1", "--since", "0"])
        .assert_code(0);
    assert_eq!(run.json()["events"].as_array().unwrap().len(), 2);
}

#[test]
fn f6_comment_keeps_the_item_waiting_for_the_operator() {
    let (env, _repo) = submitted();
    review(&env, "comment", "Looks reasonable so far.");
    assert_eq!(state(&env), "needs_review");

    let run = env.mg(&["feedback", "app#1"]).assert_code(0);
    assert!(
        run.stdout.contains("the operator commented"),
        "{}",
        run.stdout
    );
}

#[test]
fn reviews_refuse_what_they_cant_record() {
    let (env, _repo) = submitted();
    let cases = [
        (
            "app%239",
            json!({"revision": 1, "verdict": "comment", "summary": "x"}),
            404,
            "item_not_found",
        ),
        (
            "app%231",
            json!({"revision": 2, "verdict": "comment", "summary": "x"}),
            404,
            "revision_not_found",
        ),
        (
            "app%231",
            json!({"revision": 1, "verdict": "comment", "summary": "  "}),
            400,
            "empty_review",
        ),
    ];
    for (item, body, status, code) in cases {
        let reply = post(&env, &format!("/api/items/{item}/reviews"), &body);
        assert_eq!(reply.status, status, "{}", reply.body);
        assert_eq!(json_of(&reply)["error"]["code"], code, "{}", reply.body);
    }
}

#[test]
fn feedback_names_the_fix_for_unknown_items_and_cursors() {
    let (env, _repo) = submitted();
    let run = env.mg(&["--json", "feedback", "app#9"]).assert_code(1);
    let error = &run.json()["error"];
    assert_eq!(error["code"], "item_not_found");
    assert_eq!(error["hint"], "mg inbox");

    let reply = post(
        &env,
        "/api/items/app%231/feedback/ack",
        &json!({"cursor": 5}),
    );
    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(json_of(&reply)["error"]["code"], "invalid_cursor");
}
