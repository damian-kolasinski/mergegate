//! F11's guide step: `mg submit --guide` sends the agent's Markdown, whose
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
