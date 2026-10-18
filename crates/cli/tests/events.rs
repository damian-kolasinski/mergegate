//! The UI's live updates: `GET /api/events/ui` tells open pages when the
//! inbox changes, and stopping the hub ends the stream.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{Value, json};
use testkit::{Env, FakeGithub, GithubNewComment, Reply, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

/// Opens the stream and reads past the response head. The hub subscribes
/// before it answers, so every event after this returns is seen.
fn open(env: &Env) -> BufReader<TcpStream> {
    let addr = env.hub_addr();
    let mut stream = TcpStream::connect(&addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(
        stream,
        "GET /api/events/ui HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\n\r\n"
    )
    .unwrap();
    let mut reader = BufReader::new(stream);
    let mut head = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        head.push(line.trim_end().to_ascii_lowercase());
    }
    assert!(head[0].starts_with("http/1.1 200"), "{head:?}");
    assert!(head.contains(&"content-type: text/event-stream".to_owned()));
    reader
}

/// The next event's data. Chunk-size lines and keep-alive comments around it
/// are skipped.
fn next_data(reader: &mut BufReader<TcpStream>) -> String {
    loop {
        let mut line = String::new();
        assert_ne!(reader.read_line(&mut line).unwrap(), 0, "stream ended");
        if let Some(data) = line.strip_prefix("data: ") {
            return data.trim_end().to_owned();
        }
    }
}

#[test]
fn f6_a_submit_tells_open_pages_the_inbox_changed() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    let mut first = open(&env);
    let mut second = open(&env);

    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Add the parser", &[("src/parser.rs", "fn parse() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);

    assert_eq!(next_data(&mut first), r#"{"type":"inbox.changed"}"#);
    assert_eq!(next_data(&mut second), r#"{"type":"inbox.changed"}"#);
}

/// The next two events, which an action that sends both leaves in order.
/// Highlighting, which the hub does in the background after a submit, may
/// finish at any time, so its event is skipped.
fn next_two(reader: &mut BufReader<TcpStream>) -> [String; 2] {
    let mut next = || loop {
        let data = next_data(reader);
        if data != r#"{"type":"highlights.changed"}"# {
            return data;
        }
    };
    [next(), next()]
}

fn post(env: &Env, path: &str, body: &Value) -> Reply {
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
    reply
}

const CHANGED: [&str; 2] = [
    r#"{"type":"item.changed","item":"app#1"}"#,
    r#"{"type":"inbox.changed"}"#,
];

#[test]
fn an_agents_question_tells_open_pages_the_inbox_changed() {
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
    let review = json!({ "revision": 1, "verdict": "changes_requested", "summary": "" });
    post(&env, "/api/items/app%231/reviews", &review);
    let mut page = open(&env);

    // The question moves app#1 under Needs you.
    let question = ["reply", "c_1", "--status", "question", "--body", "Which?"];
    env.mg(&question).assert_code(0);

    assert_eq!(next_two(&mut page), CHANGED);
}

#[test]
fn agreeing_with_a_rejection_tells_open_pages_the_inbox_changed() {
    let github = FakeGithub::start();
    let mut vars = github.env();
    // Long enough that only `mg sync` brings the comment in.
    for name in [
        "MG_GITHUB_POLL_MS",
        "MG_EXTERNAL_QUIET_MS",
        "MG_EXTERNAL_CAP_MS",
    ] {
        vars.push((name.to_owned(), "600000".to_owned()));
    }
    let env = Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(vars);
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
    let rename = GithubNewComment::issue("alice", "Rename it.");
    github.comment("acme/app", 1, rename);
    env.mg(&["--json", "sync", "app#1"]).assert_code(0);
    post(&env, "/api/comments/c_1/vote", &json!({ "vote": "up" }));
    env.mg(&["--json", "sync", "app#1"]).assert_code(0);
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    let comment = feedback["events"][0]["comments"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let reject = ["reply", &comment, "--status", "rejected", "--body", "No."];
    env.mg(&reject).assert_code(0);
    let mut page = open(&env);

    // The rejection no longer waits on the operator.
    post(&env, &format!("/api/comments/{comment}/agree"), &json!({}));

    assert_eq!(next_two(&mut page), CHANGED);
}

#[test]
fn stopping_the_hub_ends_open_streams() {
    let env = env();
    env.start_hub();
    let mut stream = open(&env);

    env.mg(&["--json", "hub", "stop"]).assert_code(0);

    // Read to the end: a stream the hub kept open would hit the timeout.
    let mut rest = Vec::new();
    stream.read_to_end(&mut rest).unwrap();
}
