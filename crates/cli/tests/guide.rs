//! The review guide: `mg submit --guide` sends the agent's Markdown, whose
//! `## Files` list orders the files to review, and the hub serves it with
//! the item.

use std::fs;

use serde_json::{Value, json};
use testkit::{Env, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn item(env: &Env, id: &str) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!(
            "GET /api/items/{} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n",
            id.replace('#', "%23")
        ),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

/// The paths of the item's files in the order served, with their reasons.
fn order(item: &Value) -> Vec<(String, Value)> {
    item["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["path"].as_str().unwrap().to_owned(), f["reason"].clone()))
        .collect()
}

const GUIDE: &str = "Splits parsing out of main.

## Files

- `src/parser.rs` — the new parser, start here
- `src/old.rs` — gone already
";

#[test]
fn f11_the_guide_orders_the_files_and_is_kept() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["project", "add"]).assert_code(0);
    env.mg_at(repo.root(), &["init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "parser"]);
    repo.commit_files(
        "Add the parser",
        &[
            ("src/main.rs", "fn main() {}\n"),
            ("src/parser.rs", "fn parse() {}\n"),
        ],
    );
    let dir = tempfile::tempdir().unwrap();
    let guide = dir.path().join("guide.md");
    fs::write(&guide, GUIDE).unwrap();

    let text = env
        .mg_at(repo.root(), &["submit", "--guide", guide.to_str().unwrap()])
        .assert_code(0)
        .stdout;
    assert!(
        text.ends_with("\nwarning: `src/old.rs` isn't changed in this revision\n"),
        "{text}"
    );
    let served = item(&env, "app#1");
    assert_eq!(served["guide"], GUIDE);
    assert_eq!(
        order(&served),
        [
            (
                "src/parser.rs".to_owned(),
                json!("the new parser, start here")
            ),
            ("src/main.rs".to_owned(), Value::Null),
        ]
    );

    // A revision without one keeps it, and warns again.
    repo.commit_files("Use it", &[("src/main.rs", "fn main() { parse() }\n")]);
    let r2 = env
        .mg_at(repo.root(), &["--json", "submit"])
        .assert_code(0)
        .json();
    assert_eq!(r2["item"]["revision"]["number"], 2);
    assert_eq!(
        r2["warnings"],
        json!(["`src/old.rs` isn't changed in this revision"])
    );
    assert_eq!(item(&env, "app#1")["guide"], GUIDE);

    // An empty one, even at the same head, removes it.
    let cleared = env
        .mg_at(repo.root(), &["--json", "submit", "--guide", "/dev/null"])
        .assert_code(0)
        .json();
    assert_eq!(cleared["new_revision"], false);
    assert_eq!(cleared["warnings"], json!([]));
    let served = item(&env, "app#1");
    assert_eq!(served["guide"], Value::Null);
    assert_eq!(order(&served)[0].0, "src/main.rs");
}

#[test]
fn f11_an_unreadable_guide_stops_the_submit() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["project", "add"]).assert_code(0);
    env.mg_at(repo.root(), &["init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "parser"]);
    repo.commit_files("Add the parser", &[("src/parser.rs", "fn parse() {}\n")]);

    let refused = env
        .mg_at(
            repo.root(),
            &["--json", "submit", "--guide", "no-such-guide.md"],
        )
        .assert_code(1)
        .json();
    assert_eq!(refused["error"]["code"], "guide_unreadable");
    let inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    assert_eq!(inbox["needs_you"], json!([]));
}

/// A `ui` POST to the hub, as the operator's review screen sends it.
fn post(env: &Env, path: &str, body: &Value) {
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
}

#[test]
fn f11_a_guide_whose_intent_names_the_review_is_warned() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["project", "add"]).assert_code(0);
    env.mg_at(repo.root(), &["init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "parser"]);
    repo.commit_files("Add the parser", &[("src/parser.rs", "fn parse() {}\n")]);
    env.mg_at(repo.root(), &["submit"]).assert_code(0);
    let draft = json!({
        "revision": 1, "path": "src/parser.rs", "side": "new",
        "start_line": 1, "line": 1, "body": "Test it.",
    });
    post(&env, "/api/items/app%231/drafts", &draft);
    post(
        &env,
        "/api/items/app%231/reviews",
        &json!({ "revision": 1, "verdict": "changes_requested", "summary": "" }),
    );

    repo.commit_files(
        "Test it",
        &[("src/parser.rs", "fn parse() {}\n#[test]\nfn t() {}\n")],
    );
    let dir = tempfile::tempdir().unwrap();
    let guide = dir.path().join("guide.md");
    fs::write(
        &guide,
        "r2 adds the tests asked for in c_1.\n\n## Files\n\n- `src/parser.rs` — for c_1\n",
    )
    .unwrap();
    let text = env
        .mg_at(repo.root(), &["submit", "--guide", guide.to_str().unwrap()])
        .assert_code(0)
        .stdout;
    assert!(
        text.ends_with(
            "\nwarning: the guide's intent names `r2`, `c_1` from the review, but it becomes \
             the commit message when the item merges: describe the change as a whole\n"
        ),
        "{text}"
    );

    // Rewritten at the same head, it replaces the guide, and says so.
    fs::write(
        &guide,
        "Tests the parser.\n\n## Files\n\n- `src/parser.rs` — the test\n",
    )
    .unwrap();
    let text = env
        .mg_at(repo.root(), &["submit", "--guide", guide.to_str().unwrap()])
        .assert_code(0)
        .stdout;
    let head = repo.rev("HEAD");
    assert_eq!(
        text,
        format!(
            "app#1 r2 is already at {}: Add the parser\n  its guide is now the one sent\n",
            &head[..12]
        )
    );
    assert_eq!(
        item(&env, "app#1")["guide"],
        "Tests the parser.\n\n## Files\n\n- `src/parser.rs` — the test\n"
    );
}
