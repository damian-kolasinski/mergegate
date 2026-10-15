//! Re-reviewing a revision by what changed since the last review (F7, F13):
//! the hub diffs each revision against every earlier one when it's submitted,
//! leaving out what the target changed meanwhile, and viewed marks follow the
//! files that didn't change.

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

fn get(env: &Env, path: &str) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    json_of(&reply)
}

fn json_of(reply: &Reply) -> Value {
    serde_json::from_str(&reply.body).unwrap_or_else(|e| panic!("{e}: {}", reply.body))
}

fn mark(env: &Env, revision: u32, path: &str, viewed: bool) {
    let body = json!({ "revision": revision, "path": path, "viewed": viewed });
    let reply = send(env, "PUT", "/api/items/app%231/viewed", &body);
    assert_eq!(reply.status, 204, "{}", reply.body);
}

/// `(path, interdiff, viewed)` of each file of `diff`.
fn files(diff: &Value) -> Vec<(String, Value, bool)> {
    diff["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let path = f["path"].as_str().unwrap().to_owned();
            (path, f["interdiff"].clone(), f["viewed"].as_bool().unwrap())
        })
        .collect()
}

fn file(path: &str, interdiff: Value, viewed: bool) -> (String, Value, bool) {
    (path.into(), interdiff, viewed)
}

/// `app#1` r1 changes `lib.rs` and `db.rs` and adds `login.rs`. The operator
/// views `lib.rs` and `login.rs` and requests changes. Then `main` changes
/// the README and `db.rs` where r1 did, and the agent rebases onto it,
/// resolving `db.rs` and changing `lib.rs` again, as r2.
fn rebased() -> (Env, TestRepo) {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files(
        "Base",
        &[
            ("README", "title\n"),
            ("src/db.rs", "db\n"),
            ("src/lib.rs", "lib\n"),
        ],
    );
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files(
        "Login",
        &[
            ("src/db.rs", "db login\n"),
            ("src/lib.rs", "lib\nmod login;\n"),
            ("src/login.rs", "fn login() {}\n"),
        ],
    );
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    repo.git(["tag", "r1"]);

    mark(&env, 1, "src/lib.rs", true);
    mark(&env, 1, "src/login.rs", true);
    let review = json!({ "revision": 1, "verdict": "changes_requested", "summary": "Fix it" });
    let reply = send(&env, "POST", "/api/items/app%231/reviews", &review);
    assert_eq!(reply.status, 201, "{}", reply.body);

    repo.checkout("main");
    repo.commit_files(
        "Upstream",
        &[("README", "title\nmore\n"), ("src/db.rs", "db upstream\n")],
    );
    repo.checkout("feature");
    repo.git(["reset", "--quiet", "--hard", "main"]);
    repo.commit_files(
        "Login",
        &[
            ("src/db.rs", "db upstream login\n"),
            ("src/lib.rs", "lib\nmod login;\nmod token;\n"),
            ("src/login.rs", "fn login() {}\n"),
        ],
    );
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    (env, repo)
}

#[test]
fn f7_shows_what_changed_since_the_last_review_over_a_moved_base() {
    let (env, repo) = rebased();

    let diff = get(&env, "/api/items/app%231");
    assert_eq!(diff["item"]["revision"]["number"], 2);
    assert_eq!(diff["since"], 1);
    assert_eq!(diff["last_reviewed"], 1);
    assert_eq!(diff["interdiffs"], json!([1]));
    // The README moved with main alone, so it isn't there; login.rs's mark
    // came along, as r2 left it as r1 had it.
    assert_eq!(
        files(&diff),
        [
            file("src/db.rs", json!("unseparated"), false),
            file("src/lib.rs", json!("changed"), false),
            file("src/login.rs", json!("unchanged"), true),
        ]
    );
    let lib = &diff["files"][1];
    // From r1's lib.rs, which main left alone, to r2's.
    assert_eq!(lib["old_blob"], repo.rev("r1:src/lib.rs"));
    assert_eq!(lib["new_blob"], repo.rev("feature:src/lib.rs"));
    assert_eq!(
        lib["content"]["hunks"][0]["lines"],
        json!([" lib", " mod login;", "+mod token;"])
    );
    // An unseparated file shows r2's whole diff.
    let db = &diff["files"][0];
    assert_eq!(db["old_blob"], repo.rev("main:src/db.rs"));
    assert_eq!(db["new_blob"], repo.rev("feature:src/db.rs"));

    let all = get(&env, "/api/items/app%231?since=all");
    assert_eq!(all["since"], Value::Null);
    // Against the base, login.rs declares a function, which ranks it first.
    assert_eq!(
        files(&all),
        [
            file("src/login.rs", Value::Null, true),
            file("src/db.rs", Value::Null, false),
            file("src/lib.rs", Value::Null, false),
        ]
    );
    let since = get(&env, "/api/items/app%231?since=1");
    assert_eq!(since["files"], diff["files"]);

    let inbox = get(&env, "/api/inbox");
    assert_eq!(
        inbox["needs_you"][0]["changed_since"],
        json!({ "revision": 1, "files": 2 })
    );
    let text = env.mg(&["inbox"]).assert_code(0).stdout;
    assert!(text.contains(" · 2 changed since r1)"), "{text}");
}

#[test]
fn f7_viewed_marks_follow_a_file_while_it_is_unchanged() {
    let (env, _repo) = rebased();
    let viewed = |revision: u32| -> Vec<String> {
        let diff = get(&env, "/api/items/app%231?since=all");
        assert_eq!(diff["item"]["revision"]["number"], revision);
        files(&diff)
            .into_iter()
            .filter(|(_, _, viewed)| *viewed)
            .map(|(path, ..)| path)
            .collect()
    };
    assert_eq!(viewed(2), ["src/login.rs"]);

    // Unmarking on r1 reaches r2, where login.rs is unchanged; marking
    // lib.rs on r1 doesn't, as r2 changed it.
    mark(&env, 1, "src/login.rs", false);
    mark(&env, 1, "src/lib.rs", true);
    assert_eq!(viewed(2), Vec::<String>::new());
    mark(&env, 1, "src/login.rs", true);
    assert_eq!(viewed(2), ["src/login.rs"]);
}

#[test]
fn f13_a_review_and_approval_stay_on_the_revision_reviewed() {
    let (env, _repo) = rebased();
    // r1 is what the operator had on screen when r2 came in: a review still
    // goes to it, but only the newest revision merges.
    let review = json!({ "revision": 1, "verdict": "comment", "summary": "Also this" });
    let reply = send(&env, "POST", "/api/items/app%231/reviews", &review);
    assert_eq!(reply.status, 201, "{}", reply.body);
    let approve = json!({ "revision": 1, "summary": "" });
    let reply = send(&env, "POST", "/api/items/app%231/approve", &approve);
    assert_eq!(reply.status, 409, "{}", reply.body);
    assert_eq!(json_of(&reply)["error"]["code"], "stale_revision");

    let inbox = get(&env, "/api/inbox");
    assert_eq!(
        inbox["needs_you"][0]["changed_since"]["revision"], 1,
        "{inbox}"
    );
}

#[test]
fn since_is_a_revision_or_all() {
    let (env, _repo) = rebased();
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!(
            "GET /api/items/app%231?since=r1 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        ),
    );
    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(json_of(&reply)["error"]["code"], "invalid_since");
    // Not earlier than the newest: all changes.
    let diff = get(&env, "/api/items/app%231?since=2");
    assert_eq!(diff["since"], Value::Null);
}

#[test]
fn f7_a_comment_goes_on_lines_the_interdiff_shows() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    let lines = |changed: &[u32]| -> String {
        (1..=30)
            .map(|n| match changed.contains(&n) {
                true => format!("changed {n}\n"),
                false => format!("line {n}\n"),
            })
            .collect()
    };
    repo.commit_files("Base", &[("f.txt", &lines(&[]))]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Both", &[("f.txt", &lines(&[2, 25])), ("old.rs", "old\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    // r2 keeps line 2's change, takes back line 25's and drops old.rs.
    repo.git(["rm", "--quiet", "old.rs"]);
    repo.commit_files("One", &[("f.txt", &lines(&[2]))]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);

    let draft = |path: &str, line: u32| {
        let body = json!({
            "revision": 2, "path": path, "side": "new",
            "start_line": line, "line": line, "body": "Why?",
        });
        send(&env, "POST", "/api/items/app%231/drafts", &body)
    };
    // Line 25 is back as the base has it, so only since r1 shows it.
    let reply = draft("f.txt", 25);
    assert_eq!(reply.status, 201, "{}", reply.body);
    assert_eq!(json_of(&reply)["quote"], "line 25");
    let reply = draft("f.txt", 15);
    assert_eq!(reply.status, 400, "{}", reply.body);
    assert_eq!(json_of(&reply)["error"]["code"], "invalid_lines");
    // r2 doesn't change old.rs, so it has no lines to comment on.
    let reply = draft("old.rs", 1);
    assert_eq!(reply.status, 404, "{}", reply.body);
    assert_eq!(json_of(&reply)["error"]["code"], "file_not_found");
}
