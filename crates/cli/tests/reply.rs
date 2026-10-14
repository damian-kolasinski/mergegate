//! The agent answering the operator's comments (F7's last step): `mg reply`
//! marks a sent comment addressed, rejected or a question, and the item shows
//! the reply on the comment's thread. The operator answers back with draft
//! replies, which the next review sends.

use serde_json::{Value, json};
use testkit::{Env, Reply, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
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

fn post(env: &Env, path: &str, body: &Value) -> Value {
    let reply = send(env, "POST", path, body);
    assert!(reply.status < 300, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

/// `GET /api/items/app%231`: the item with its comments and replies.
fn item(env: &Env) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!("GET /api/items/app%231 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    serde_json::from_str(&reply.body).unwrap()
}

/// `app#1` with c_1 sent in a review and c_2 still a draft.
fn reviewed() -> (Env, TestRepo) {
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
    let draft = json!({
        "revision": 1, "path": "src/login.rs", "side": "new",
        "start_line": 1, "line": 1, "body": "Check the password.",
    });
    post(&env, "/api/items/app%231/drafts", &draft);
    post(
        &env,
        "/api/items/app%231/reviews",
        &json!({ "revision": 1, "verdict": "changes_requested", "summary": "" }),
    );
    post(&env, "/api/items/app%231/drafts", &draft);
    (env, repo)
}

#[test]
fn f7_replies_reach_the_comments_thread() {
    let (env, repo) = reviewed();
    let run = env
        .mg(&["--json", "reply", "c_1", "--status", "addressed"])
        .assert_code(0);
    let addressed = run.json();
    assert_eq!(
        (
            &addressed["comment"],
            &addressed["author"],
            &addressed["status"],
            &addressed["body"],
            &addressed["addressed_in"],
        ),
        (
            &json!("c_1"),
            &json!("agent"),
            &json!("addressed"),
            &json!(""),
            &json!(null),
        )
    );
    let text = env
        .mg(&[
            "reply",
            "c_1",
            "--status",
            "question",
            "--body",
            "Empty or missing too?",
        ])
        .assert_code(0);
    assert_eq!(text.stdout.trim(), "marked c_1 a question");

    // The next revision is the one that addressed it.
    repo.commit_files("Check", &[("src/login.rs", "fn login(p: &str) {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    let replies = item(&env)["replies"].clone();
    assert_eq!(
        replies
            .as_array()
            .unwrap()
            .iter()
            .map(|reply| (&reply["status"], &reply["body"], &reply["addressed_in"]))
            .collect::<Vec<_>>(),
        [
            (&json!("addressed"), &json!(""), &json!(2)),
            (
                &json!("question"),
                &json!("Empty or missing too?"),
                &json!(null)
            ),
        ]
    );
}

#[test]
fn replies_answer_only_sent_comments_and_say_why() {
    let (env, _repo) = reviewed();
    let refused = |args: &[&str], code: &str| {
        let mut all = vec!["--json", "reply"];
        all.extend_from_slice(args);
        let run = env.mg(&all);
        assert_eq!(run.json()["error"]["code"], code, "{run:?}");
        assert_ne!(run.code, 0, "{run:?}");
        run.json()["error"]["hint"].clone()
    };
    refused(&["c_2", "--status", "addressed"], "comment_not_found");
    refused(&["c_9", "--status", "addressed"], "comment_not_found");
    refused(&["17", "--status", "addressed"], "invalid_comment");
    assert_eq!(
        refused(&["c_1", "--status", "rejected"], "empty_reply"),
        "mg reply c_1 --status rejected --body WHY"
    );
    refused(
        &["c_1", "--status", "question", "--body", "  "],
        "empty_reply",
    );
    assert_eq!(item(&env)["replies"], json!([]));
}

#[test]
fn f7_the_operators_replies_go_out_with_the_next_review() {
    let (env, _repo) = reviewed();
    env.mg(&[
        "reply",
        "c_1",
        "--status",
        "question",
        "--body",
        "Empty or missing?",
    ])
    .assert_code(0);
    env.mg(&["feedback", "app#1"]).assert_code(0);
    let kept = post(
        &env,
        "/api/comments/c_1/drafts",
        &json!({ "body": "Both." }),
    );
    let dropped = post(
        &env,
        "/api/comments/c_1/drafts",
        &json!({ "body": "Neither." }),
    );
    let draft = |reply: &Value| format!("/api/comments/c_1/drafts/{}", reply["id"]);
    let edited = send(
        &env,
        "PUT",
        &draft(&kept),
        &json!({ "body": " Both, please.\n" }),
    );
    assert_eq!(edited.status, 200, "{}", edited.body);
    let deleted = send(&env, "DELETE", &draft(&dropped), &json!(null));
    assert_eq!(deleted.status, 204, "{}", deleted.body);
    let bodies = |replies: &Value| {
        replies
            .as_array()
            .unwrap()
            .iter()
            .map(|reply| (reply["author"].clone(), reply["body"].clone()))
            .collect::<Vec<_>>()
    };
    let shown = item(&env);
    assert_eq!(
        bodies(&shown["draft_replies"]),
        [(json!("operator"), json!("Both, please."))]
    );
    assert_eq!(
        bodies(&shown["replies"]),
        [(json!("agent"), json!("Empty or missing?"))]
    );

    let sent = post(
        &env,
        "/api/items/app%231/reviews",
        &json!({ "revision": 1, "verdict": "comment", "summary": "" }),
    );
    assert_eq!(
        bodies(&sent["replies"]),
        [(json!("operator"), json!("Both, please."))]
    );
    let text = env.mg(&["feedback", "app#1"]).assert_code(0);
    assert_eq!(
        text.stdout.trim(),
        "[2] app#1 r1: the operator commented\n\n  \
         c_2 src/login.rs:1\n    > fn login() {}\n    Check the password.\n\n  \
         reply to c_1\n    Both, please.\n\n\
         cursor 2 (mg wait --since 2 replays from here)"
    );
    let shown = item(&env);
    assert_eq!(shown["draft_replies"], json!([]));
    assert_eq!(
        bodies(&shown["replies"]),
        [
            (json!("agent"), json!("Empty or missing?")),
            (json!("operator"), json!("Both, please."))
        ]
    );

    // Sent, it stays as the agent read it.
    let late = send(
        &env,
        "PUT",
        &draft(&kept),
        &json!({ "body": "Never mind." }),
    );
    assert_eq!(late.status, 409, "{}", late.body);
}

#[test]
fn only_sent_comments_take_the_operators_replies() {
    let (env, _repo) = reviewed();
    let refused = |path: &str, body: &str| {
        let reply = send(&env, "POST", path, &json!({ "body": body }));
        let error: Value = serde_json::from_str(&reply.body).unwrap();
        (reply.status, error["error"]["code"].clone())
    };
    assert_eq!(
        refused("/api/comments/c_2/drafts", "Hm."),
        (404, json!("comment_not_found"))
    );
    assert_eq!(
        refused("/api/comments/c_1/drafts", " "),
        (400, json!("empty_reply"))
    );
    let missing = send(&env, "DELETE", "/api/comments/c_1/drafts/7", &json!(null));
    assert_eq!(missing.status, 404, "{}", missing.body);
    assert_eq!(item(&env)["draft_replies"], json!([]));
}
