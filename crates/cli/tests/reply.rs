//! The agent answering the operator's comments (F7's last step): `mg reply`
//! marks a sent comment addressed, rejected or a question, and the item shows
//! the reply on the comment's thread.

use serde_json::{Value, json};
use testkit::{Env, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn post(env: &Env, path: &str, body: &Value) -> Value {
    let addr = env.hub_addr();
    let body = body.to_string();
    let reply = http_reply(
        &addr,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
             {}: ui\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            api::CLIENT_HEADER,
            body.len(),
        ),
    );
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
