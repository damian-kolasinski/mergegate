//! The operator's review reaching the agent (F6's last steps, F7, F3's
//! `mg wait`): drafts on lines of the diff go out with the review, the hub
//! logs it, `mg wait` and `mg feedback` deliver it once, and `--since`
//! replays it.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use testkit::{Env, Reply, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn post(env: &Env, path: &str, body: &Value) -> Reply {
    send(env, "POST", path, body)
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

/// `GET /api/items/app%231`: the item with its files and comments.
fn item(env: &Env) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!("GET /api/items/app%231 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    json_of(&reply)
}

fn state(env: &Env) -> Value {
    item(env)["item"]["state"].clone()
}

/// Drafts a comment on lines of `src/login.rs` in r1.
fn draft(env: &Env, side: &str, start_line: u32, line: u32, body: &str) -> Reply {
    post(
        env,
        "/api/items/app%231/drafts",
        &json!({
            "revision": 1, "path": "src/login.rs", "side": side,
            "start_line": start_line, "line": line, "body": body,
        }),
    )
}

#[test]
fn f7_wait_returns_the_review_once() {
    let (env, _repo) = submitted();
    let submitted = review(
        &env,
        "changes_requested",
        "Handle the empty password.\nThanks!",
    );
    assert_eq!(submitted["verdict"], "changes_requested");
    assert_eq!(submitted["revision"], 1);
    assert_eq!(state(&env), "changes_requested");

    let run = env.mg(&["--json", "wait", "app#1"]).assert_code(0);
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
            "comments": [],
            "replies": [],
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
        "[1] review.submitted app#1 r1: the operator requested changes\n  Handle the empty password.\n\n\
         cursor 1 (mg feedback app#1 --since 1 continues after it)"
    );
}

#[test]
fn f7_since_replays_delivered_events() {
    let (env, _repo) = submitted();
    review(&env, "comment", "First look.");
    review(&env, "changes_requested", "Second look.");
    env.mg(&["--json", "feedback", "app#1"]).assert_code(0);

    let replay = env
        .mg(&["--json", "wait", "app#1", "--since", "1"])
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
fn f7_a_wait_since_the_printed_cursor_delivers_what_it_printed() {
    let (env, _repo) = submitted();
    review(&env, "comment", "First look.");
    env.mg(&["--json", "wait", "app#1"]).assert_code(0);
    review(&env, "changes_requested", "Second look.");

    // As the cursor line after the first wait suggests.
    let next = env
        .mg(&["--json", "wait", "app#1", "--since", "1"])
        .assert_code(0)
        .json();
    assert_eq!(next["events"][0]["summary"], "Second look.");
    let again = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    assert_eq!(again["events"], json!([]));
}

#[test]
fn f7_a_replay_past_the_delivered_cursor_skips_nothing() {
    let (env, _repo) = submitted();
    review(&env, "comment", "First look.");
    review(&env, "changes_requested", "Second look.");

    let replay = env
        .mg(&["--json", "feedback", "app#1", "--since", "1"])
        .assert_code(0)
        .json();
    assert_eq!(replay["events"].as_array().unwrap().len(), 1);
    let new = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    assert_eq!(new["events"].as_array().unwrap().len(), 2);
}

#[test]
fn f6_comment_keeps_the_item_waiting_for_the_operator() {
    let (env, _repo) = submitted();
    review(&env, "comment", "Looks reasonable so far.");
    assert_eq!(state(&env), "needs_review");

    let run = env.mg(&["wait", "app#1"]).assert_code(0);
    assert!(
        run.stdout.contains("the operator commented"),
        "{}",
        run.stdout
    );
}

#[test]
fn f3_wait_times_out_with_75() {
    let (env, _repo) = submitted();
    let started = Instant::now();
    let run = env.mg(&["wait", "app#1", "--timeout", "1"]).assert_code(75);
    assert!(started.elapsed() >= Duration::from_secs(1));
    assert_eq!(
        run.stdout.trim(),
        "no feedback on app#1 yet; call mg wait again"
    );
}

#[test]
fn f3_wait_wakes_when_the_operator_submits() {
    let (env, _repo) = submitted();
    let started = Instant::now();
    let run = std::thread::scope(|scope| {
        let waiting = scope.spawn(|| env.mg(&["--json", "wait", "app#1", "--timeout", "30"]));
        std::thread::sleep(Duration::from_millis(500));
        review(&env, "changes_requested", "Wake up.");
        waiting.join().unwrap()
    });
    let feedback = run.assert_code(0).json();
    assert_eq!(feedback["events"][0]["summary"], "Wake up.");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn f7_a_cursor_past_the_items_events_reads_from_its_newest() {
    let (env, _repo) = submitted();
    review(&env, "comment", "First look.");
    let past = env
        .mg(&["--json", "feedback", "app#1", "--since", "99"])
        .assert_code(0)
        .json();
    assert_eq!((&past["events"], &past["cursor"]), (&json!([]), &json!(1)));

    // Events logged while it waits come after the item's newest, not 99.
    let run = std::thread::scope(|scope| {
        let waiting =
            scope.spawn(|| env.mg(&["--json", "wait", "app#1", "--since", "99", "--timeout", "5"]));
        std::thread::sleep(Duration::from_millis(500));
        review(&env, "changes_requested", "Wake up.");
        waiting.join().unwrap()
    });
    let woken = run.assert_code(0).json();
    assert_eq!(woken["events"][0]["summary"], "Wake up.");
    assert_eq!(woken["cursor"], 2);
}

#[test]
fn f3_stopping_the_hub_ends_a_pending_wait() {
    let (env, _repo) = submitted();
    let started = Instant::now();
    let run = std::thread::scope(|scope| {
        let waiting = scope.spawn(|| env.mg(&["--json", "wait", "app#1", "--timeout", "30"]));
        std::thread::sleep(Duration::from_millis(500));
        env.mg(&["hub", "stop"]).assert_code(0);
        waiting.join().unwrap()
    });
    let run = run.assert_code(69);
    assert_eq!(run.json()["error"]["code"], "hub_unavailable", "{run:?}");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn f6_drafts_reach_the_agent_with_the_lines_they_are_on() {
    let (env, _repo) = submitted();
    let written = draft(&env, "new", 1, 1, "Check the password.");
    assert_eq!(written.status, 201, "{}", written.body);
    let expected = json!({
        "id": "c_1", "revision": 1, "path": "src/login.rs", "side": "new",
        "start_line": 1, "line": 1, "body": "Check the password.", "quote": "fn login() {}",
    });
    assert_eq!(json_of(&written), expected);
    assert_eq!(item(&env)["drafts"], json!([expected]));

    // A review with comments needs no summary.
    let sent = review(&env, "changes_requested", "");
    assert_eq!(sent["comments"], json!([expected]));
    let shown = item(&env);
    assert_eq!(
        (&shown["drafts"], &shown["comments"]),
        (&json!([]), &json!([expected]))
    );

    let feedback = env
        .mg(&["--json", "feedback", "app#1", "--since", "0"])
        .assert_code(0);
    assert_eq!(feedback.json()["events"][0]["comments"], json!([expected]));
    let text = env
        .mg(&["feedback", "app#1", "--since", "0"])
        .assert_code(0);
    assert_eq!(
        text.stdout.trim(),
        "[1] review.submitted app#1 r1: the operator requested changes\n\n  \
         c_1 src/login.rs:1\n    > fn login() {}\n    Check the password.\n\n\
         cursor 1 (mg feedback app#1 --since 1 continues after it)"
    );
}

#[test]
fn f6_drafts_change_until_a_review_sends_them() {
    let (env, _repo) = submitted();
    let first = json_of(&draft(&env, "new", 1, 1, "First."));
    let second = json_of(&draft(&env, "new", 1, 1, "Second."));
    let edited = send(
        &env,
        "PUT",
        "/api/items/app%231/drafts/c_1",
        &json!({"body": "First, better."}),
    );
    assert_eq!(edited.status, 200, "{}", edited.body);
    assert_eq!(json_of(&edited)["body"], "First, better.");
    let deleted = send(
        &env,
        "DELETE",
        "/api/items/app%231/drafts/c_2",
        &json!(null),
    );
    assert_eq!(deleted.status, 204, "{}", deleted.body);
    let drafts = item(&env)["drafts"].clone();
    assert_eq!(drafts.as_array().unwrap().len(), 1);
    assert_eq!(drafts[0]["id"], first["id"]);
    assert_eq!(second["id"], "c_2");

    review(&env, "comment", "");
    let late = send(
        &env,
        "PUT",
        "/api/items/app%231/drafts/c_1",
        &json!({"body": "Too late."}),
    );
    assert_eq!(late.status, 409, "{}", late.body);
    assert_eq!(json_of(&late)["error"]["code"], "comment_sent");
}

#[test]
fn drafts_refuse_lines_they_cant_quote() {
    let (env, repo) = submitted();
    repo.commit_files("Logo", &[("logo.png", "PNG\0\0")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    let on_revision = |revision: u32, path: &str, side: &str, line: u32, body: &str| {
        post(
            &env,
            "/api/items/app%231/drafts",
            &json!({
                "revision": revision, "path": path, "side": side,
                "start_line": line, "line": line, "body": body,
            }),
        )
    };
    let on = |path: &str, side: &str, line: u32, body: &str| on_revision(2, path, side, line, body);
    let cases = [
        (
            on_revision(3, "src/login.rs", "new", 1, "x"),
            404,
            "revision_not_found",
        ),
        (on("src/login.rs", "new", 2, "x"), 400, "invalid_lines"),
        (on("src/login.rs", "old", 1, "x"), 400, "invalid_lines"),
        (on("src/other.rs", "new", 1, "x"), 404, "file_not_found"),
        (on("logo.png", "new", 1, "x"), 400, "not_text"),
        (on("src/login.rs", "new", 1, " "), 400, "empty_comment"),
        (
            send(
                &env,
                "PUT",
                "/api/items/app%231/drafts/17",
                &json!({"body": "x"}),
            ),
            400,
            "invalid_comment",
        ),
        (
            send(
                &env,
                "DELETE",
                "/api/items/app%231/drafts/c_9",
                &json!(null),
            ),
            404,
            "draft_not_found",
        ),
        (
            send(
                &env,
                "DELETE",
                "/api/items/app%239/drafts/c_1",
                &json!(null),
            ),
            404,
            "item_not_found",
        ),
    ];
    for (reply, status, code) in cases {
        assert_eq!(reply.status, status, "{}", reply.body);
        assert_eq!(json_of(&reply)["error"]["code"], code, "{}", reply.body);
    }
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
fn wait_names_the_fix_for_unknown_items_and_cursors() {
    let (env, _repo) = submitted();
    let run = env.mg(&["--json", "wait", "app#9"]).assert_code(1);
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
