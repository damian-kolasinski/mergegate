//! Review time: the UI reports how long the operator was active on an item,
//! and the item's closing event carries the totals, per turn and per
//! revision, for its agent's telemetry, as both the item's feedback and the
//! hub's event log read it.

use serde_json::{Value, json};
use testkit::{Env, Reply, TestRepo, http_reply};

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

/// `app#1` with one revision, waiting for the operator.
fn submitted() -> (Env, TestRepo) {
    let env = Env::new(env!("CARGO_BIN_EXE_mg"))
        .with_vars(vec![("MG_TARGET_WATCH".into(), "off".into())]);
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    (env, repo)
}

/// The agent's next revision.
fn revise(env: &Env, repo: &TestRepo) {
    repo.commit_files("Add logout", &[("src/logout.rs", "fn logout() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
}

fn spend(env: &Env, revision: u32, ms: u64) -> Reply {
    let body = json!({ "revision": revision, "ms": ms });
    send(env, "POST", "/api/items/app%231/review-time", &body)
}

fn review(env: &Env, revision: u32, verdict: &str) {
    let body = json!({ "revision": revision, "verdict": verdict, "summary": "Fix it." });
    let reply = send(env, "POST", "/api/items/app%231/reviews", &body);
    assert_eq!(reply.status, 201, "{}", reply.body);
}

fn closing(env: &Env, kind: &str) -> Value {
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    let event = feedback["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == kind)
        .unwrap_or_else(|| panic!("no {kind} in {feedback}"))
        .clone();
    // The hub's log reports it the same.
    let log = env
        .mg(&["--json", "events", "--since", "0"])
        .assert_code(0)
        .json();
    let logged = log["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == kind)
        .unwrap();
    assert_eq!(logged["stats"], event["stats"]);
    event
}

/// `(turn, revision, verdict, review_ms)` of each turn.
fn turns(stats: &Value) -> Vec<(Value, Value, Value, Value)> {
    stats["turns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["turn"].clone(),
                t["revision"].clone(),
                t["verdict"].clone(),
                t["review_ms"].clone(),
            )
        })
        .collect()
}

#[test]
fn a_merge_carries_the_operators_review_time_per_turn_and_revision() {
    let (env, repo) = submitted();
    assert_eq!(spend(&env, 1, 30_000).status, 204);
    review(&env, 1, "changes_requested");
    revise(&env, &repo);
    assert_eq!(spend(&env, 2, 12_000).status, 204);
    assert_eq!(spend(&env, 2, 3_000).status, 204);

    let body = json!({ "revision": 2, "summary": "" });
    assert_eq!(
        send(&env, "POST", "/api/items/app%231/approve", &body).status,
        200
    );

    let stats = &closing(&env, "item.merged")["stats"];
    assert_eq!(stats["review_ms"], 45_000, "{stats}");
    assert_eq!(
        turns(stats),
        [
            (
                json!(1),
                json!(1),
                json!("changes_requested"),
                json!(30_000)
            ),
            (json!(2), json!(2), json!("approved"), json!(15_000)),
        ],
        "{stats}"
    );
    let per: Vec<_> = stats["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["revision"].clone(), r["review_ms"].clone()))
        .collect();
    assert_eq!(
        per,
        [(json!(1), json!(30_000)), (json!(2), json!(15_000))],
        "{stats}"
    );
    let opened = stats["opened_at"].as_str().unwrap();
    let closed = stats["closed_at"].as_str().unwrap();
    let first_sent = stats["turns"][0]["sent_at"].as_str().unwrap();
    assert!(opened <= first_sent && first_sent <= closed, "{stats}");
    assert_eq!(stats["revisions"][0]["submitted_at"], opened);
}

#[test]
fn a_closed_items_review_time_stays_as_its_close_reported_it() {
    let (env, repo) = submitted();
    revise(&env, &repo);
    assert_eq!(spend(&env, 2, 5_000).status, 204);
    env.mg(&["abandon", "app#1", "--reason", "Not needed."])
        .assert_code(0);

    let refused = spend(&env, 2, 5_000);

    assert_eq!(refused.status, 409, "{}", refused.body);
    assert!(refused.body.contains("item_closed"), "{}", refused.body);
    let stats = &closing(&env, "item.abandoned")["stats"];
    assert_eq!(stats["review_ms"], 5_000, "{stats}");
    // Abandoned mid-turn: the turn under way has no review to end it.
    assert_eq!(
        turns(stats),
        [(json!(1), json!(2), Value::Null, json!(5_000))],
        "{stats}"
    );
    assert_eq!(stats["turns"][0]["sent_at"], Value::Null);
}

#[test]
fn refuses_time_on_a_revision_the_item_lacks() {
    let (env, _repo) = submitted();

    let refused = spend(&env, 2, 1_000);

    assert_eq!(refused.status, 404, "{}", refused.body);
}
