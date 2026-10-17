//! Comments on a later revision: read onto it, each comment from an
//! earlier one is placed on the lines that kept or replaced its own, or is
//! outdated, and a review of it waits for drafts that can't move until the
//! operator re-places or deletes them.

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

fn error_code(reply: &Reply) -> Value {
    json_of(reply)["error"]["code"].clone()
}

const R1: &str = "fn login(password: &str) {\n    check(password);\n    audit();\n    log();\n    \
                  store();\n    done();\n    save();\n    trace();\n}\n";

/// Lines 2 and 6 change, 4 and 8 go, the rest stay.
const R2: &str = "fn login(password: &str) {\n    verify(password);\n    audit();\n    store();\n    \
                  finish();\n    save();\n}\n";

fn draft(env: &Env, path: &str, side: &str, line: u32, body: &str) {
    let reply = send(
        env,
        "POST",
        "/api/items/app%231/drafts",
        &json!({
            "revision": 1, "path": path, "side": side,
            "start_line": line, "line": line, "body": body,
        }),
    );
    assert_eq!(reply.status, 201, "{}", reply.body);
}

fn review(env: &Env, revision: u32) -> Reply {
    let body = json!({ "revision": revision, "verdict": "changes_requested", "summary": "" });
    send(env, "POST", "/api/items/app%231/reviews", &body)
}

/// Moves draft `comment` onto lines `line..=line` of `src/login.rs` in r2.
fn place(env: &Env, comment: &str, line: u32) -> Reply {
    let body = json!({
        "revision": 2, "path": "src/login.rs", "side": "new", "start_line": line, "line": line,
    });
    let path = format!("/api/items/app%231/drafts/{comment}/anchor");
    send(env, "PUT", &path, &body)
}

/// `app#1` r1 adds `src/login.rs` and changes `src/db.rs`. The operator
/// requests changes with c_1 on login.rs line 2 and c_2 on line 4, then
/// drafts c_3 on line 3, c_4 on line 6, c_5 on line 8 and c_6 on db.rs's
/// base line 2 before the agent submits r2.
fn revised() -> (Env, TestRepo) {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("src/db.rs", "db\nopen\n")]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files(
        "Login",
        &[("src/db.rs", "db\nopen(url)\n"), ("src/login.rs", R1)],
    );
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);

    draft(&env, "src/login.rs", "new", 2, "Say what check does.");
    draft(&env, "src/login.rs", "new", 4, "Log less.");
    assert_eq!(review(&env, 1).status, 201);
    draft(&env, "src/login.rs", "new", 3, "Audit failures too.");
    draft(&env, "src/login.rs", "new", 6, "Rename it.");
    draft(&env, "src/login.rs", "new", 8, "Trace at debug.");
    draft(&env, "src/db.rs", "old", 2, "Why not here?");

    repo.commit_files("Verify", &[("src/login.rs", R2)]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    (env, repo)
}

/// `(id, placement)` of each comment in `list`.
fn placements(list: &Value) -> Vec<(String, Value)> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|c| (c["id"].as_str().unwrap().to_owned(), c["placement"].clone()))
        .collect()
}

fn placed(id: &str, placement: Value) -> (String, Value) {
    (id.into(), placement)
}

#[test]
fn f7_comments_follow_their_lines_onto_a_later_revision() {
    let (env, _repo) = revised();

    let onto = get(&env, "/api/items/app%231/conversation?onto=2");
    let changed = |line: u32| json!({ "kind": "changed", "start_line": line, "line": line });
    let moved = |line: u32| json!({ "kind": "moved", "start_line": line, "line": line });
    let outdated = json!({ "kind": "outdated" });
    assert_eq!(
        placements(&onto["comments"]),
        [placed("c_1", changed(2)), placed("c_2", outdated.clone())],
    );
    assert_eq!(
        placements(&onto["drafts"]),
        [
            placed("c_3", moved(3)),
            placed("c_4", changed(5)),
            placed("c_5", outdated),
            // The base didn't move, so its lines stay.
            placed("c_6", moved(2)),
        ],
    );
    // Every comment keeps the lines it was written on.
    assert_eq!(onto["comments"][0]["revision"], 1);
    assert_eq!(onto["comments"][0]["line"], 2);

    // Without a revision they go onto the newest; onto their own, nowhere.
    assert_eq!(get(&env, "/api/items/app%231/conversation"), onto);
    let own = get(&env, "/api/items/app%231/conversation?onto=1");
    for list in [&own["comments"], &own["drafts"]] {
        assert!(placements(list).iter().all(|(_, p)| p.is_null()));
    }
}

#[test]
fn f13_a_review_waits_for_drafts_that_cant_move() {
    let (env, _repo) = revised();

    // c_4's line changed and c_5's went: neither a review nor an approval of
    // r2 goes out without them, and nothing is sent.
    let held = review(&env, 2);
    assert_eq!(held.status, 409, "{}", held.body);
    assert_eq!(error_code(&held), "drafts_unplaced");
    let approve = json!({ "revision": 2, "summary": "" });
    let held = send(&env, "POST", "/api/items/app%231/approve", &approve);
    assert_eq!(held.status, 409, "{}", held.body);
    assert_eq!(error_code(&held), "drafts_unplaced");
    let drafts = get(&env, "/api/items/app%231/conversation")["drafts"].clone();
    assert_eq!(drafts.as_array().unwrap().len(), 4);

    // A sent comment stays where it was sent.
    let sent = place(&env, "c_1", 2);
    assert_eq!(sent.status, 409, "{}", sent.body);
    assert_eq!(error_code(&sent), "comment_sent");

    // c_4 is kept on the line that replaced its own, c_5 goes elsewhere.
    let kept = place(&env, "c_4", 5);
    assert_eq!(kept.status, 200, "{}", kept.body);
    let kept = json_of(&kept);
    assert_eq!(
        (&kept["revision"], &kept["line"], &kept["quote"]),
        (&json!(2), &json!(5), &json!("    finish();")),
    );
    assert_eq!(place(&env, "c_5", 6).status, 200);

    let reply = review(&env, 2);
    assert_eq!(reply.status, 201, "{}", reply.body);
    let run = env.mg(&["--json", "wait", "app#1"]).assert_code(0);
    let events = run.json()["events"].clone();
    let last = &events[events.as_array().unwrap().len() - 1];
    assert_eq!(last["revision"], 2);
    // The ones that moved by themselves go out on r2's lines too.
    let sent: Vec<_> = last["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["id"].clone(),
                c["path"].clone(),
                c["revision"].clone(),
                c["line"].clone(),
            )
        })
        .collect();
    assert_eq!(
        sent,
        [
            (json!("c_3"), json!("src/login.rs"), json!(2), json!(3)),
            (json!("c_4"), json!("src/login.rs"), json!(2), json!(5)),
            (json!("c_5"), json!("src/login.rs"), json!(2), json!(6)),
            (json!("c_6"), json!("src/db.rs"), json!(2), json!(2)),
        ],
    );
}
