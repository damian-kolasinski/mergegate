//! Abandoning an item (F14): its agent with `mg abandon`, or the operator
//! from the UI. Either way the other side hears of it: the agent's `mg wait`
//! prints who closed it and why and exits 3, and the UI shows it closed, its
//! diffs dropped.

use serde_json::{Value, json};
use testkit::{Env, Reply, TestRepo, git, http_reply};

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

/// `GET /api/items/app%231`: the item with its files.
fn item(env: &Env) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!("GET /api/items/app%231 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    json_of(&reply)
}

fn abandon(env: &Env, reason: Value) -> Reply {
    post(
        env,
        "/api/items/app%231/abandon",
        &json!({ "source": "operator", "reason": reason }),
    )
}

#[test]
fn f14_the_agent_abandons_its_item() {
    let (env, repo) = submitted();
    let run = env.mg(&["abandon", "app#1", "--reason", "Moved to app#2."]);
    assert_eq!(run.assert_code(0).stdout.trim(), "abandoned app#1");

    let item = item(&env);
    assert_eq!(item["item"]["state"], "abandoned");
    let files = item["files"].as_array().unwrap();
    assert_eq!(files[0]["path"], "src/login.rs");
    assert_eq!(files[0]["content"], json!({ "kind": "gone" }));
    assert_eq!(files[0]["additions"], 1);

    // It left the inbox, and its branch can be submitted again as a new item.
    let mut inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    let closed = inbox["closed"].take();
    assert_eq!(closed[0]["id"], "app#1");
    assert_eq!(closed[0]["state"], "abandoned");
    assert_eq!(
        inbox,
        json!({ "needs_you": [], "with_agents": [], "merging": [], "closed": null, "rules": [] })
    );
    repo.commit_files("Again", &[("src/login.rs", "fn login() { }\n")]);
    let again = env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    assert_eq!(again.json()["item"]["id"], "app#2");

    // Its own agent closed it, so its wait has only that to say.
    let wait = env.mg(&["wait", "app#1"]).assert_code(3);
    assert_eq!(
        wait.stdout.trim(),
        "[1] app#1 r1: its agent abandoned it\n  Moved to app#2.\n\n\
         cursor 1 (mg wait --since 1 replays from here)\n\napp#1 is abandoned"
    );
}

#[test]
fn f14_an_abandoned_items_branch_name_is_free_for_branches_under_it() {
    let (env, repo) = submitted();
    env.mg(&["abandon", "app#1"]).assert_code(0);
    repo.git(["switch", "--quiet", "--detach", "main"]);
    repo.git(["branch", "-D", "feature"]);
    repo.git(["switch", "--quiet", "-c", "feature/retry"]);
    repo.commit_files("Retry", &[("src/login.rs", "fn login() { }\n")]);
    let next = env.mg_at(repo.root(), &["--json", "submit"]).json();
    assert_eq!(next["item"]["id"], "app#2", "{next}");
}

#[test]
fn f14_the_hub_drops_closed_items_branches_left_in_staging_when_it_starts() {
    let (env, repo) = submitted();
    env.mg(&["abandon", "app#1"]).assert_code(0);
    // Where a hub that kept closed items' branches left this one.
    let staging = env.home().join("projects").join("app").join("staging.git");
    let head = repo.rev("feature");
    git(&staging, ["update-ref", "refs/heads/feature", &head]);
    env.mg(&["hub", "stop"]).assert_code(0);
    env.start_hub();

    repo.git(["switch", "--quiet", "--detach", "main"]);
    repo.git(["branch", "-D", "feature"]);
    repo.git(["switch", "--quiet", "-c", "feature/retry"]);
    repo.commit_files("Retry", &[("src/login.rs", "fn login() { }\n")]);
    let next = env.mg_at(repo.root(), &["--json", "submit"]).json();
    assert_eq!(next["item"]["id"], "app#2", "{next}");
}

#[test]
fn f14_the_operator_abandons_an_item_and_its_agent_hears_why() {
    let (env, _repo) = submitted();
    let reply = abandon(&env, json!("Not needed after all."));
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(json_of(&reply)["state"], "abandoned");

    let wait = env.mg(&["--json", "wait", "app#1"]).assert_code(3);
    let feedback = wait.json();
    assert_eq!(feedback["state"], "abandoned");
    assert_eq!(
        feedback["events"],
        json!([{
            "cursor": 1,
            "item": "app#1",
            "revision": 1,
            "source": "operator",
            "type": "item.abandoned",
            "reason": "Not needed after all.",
        }])
    );
    // Delivered once; a second wait only says it's closed.
    let again = env.mg(&["wait", "app#1"]).assert_code(3);
    assert_eq!(again.stdout.trim(), "app#1 is abandoned");
}

#[test]
fn f14_an_abandoned_item_stays_closed() {
    let (env, _repo) = submitted();
    assert_eq!(abandon(&env, Value::Null).status, 200);
    let text = env.mg(&["feedback", "app#1"]).assert_code(0);
    assert_eq!(
        text.stdout.trim(),
        "[1] app#1 r1: the operator abandoned it\n\n\
         cursor 1 (mg wait --since 1 replays from here)"
    );

    let reply = abandon(&env, Value::Null);
    assert_eq!(reply.status, 409, "{}", reply.body);
    assert_eq!(json_of(&reply)["error"]["code"], "item_closed");
    let run = env.mg(&["--json", "abandon", "app#1"]).assert_code(1);
    assert_eq!(run.json()["error"]["code"], "item_closed");
    let run = env.mg(&["--json", "abandon", "app#9"]).assert_code(1);
    assert_eq!(run.json()["error"]["code"], "item_not_found");
}
