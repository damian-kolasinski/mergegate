//! Approving an item merges it (F8): the operator's approval reaches the
//! agent, the revision lands on the target as one commit authored by the
//! operator, and a merge that can't land either goes back to the agent to
//! rebase (F9) or waits for the operator to retry or cancel it.

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

fn json_of(reply: &Reply) -> Value {
    serde_json::from_str(&reply.body).unwrap_or_else(|e| panic!("{e}: {}", reply.body))
}

/// `app#1`, adding `src/login.rs` on `feature`, waiting for the operator. The
/// operator's checkout is left on `feature`.
fn submitted() -> (Env, TestRepo) {
    let env = env();
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

fn approve(env: &Env, revision: u32, summary: &str) -> Reply {
    let body = json!({ "revision": revision, "summary": summary });
    send(env, "POST", "/api/items/app%231/approve", &body)
}

fn item(env: &Env) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!("GET /api/items/app%231 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    json_of(&reply)
}

/// A commit on `main` in the operator's repo, leaving `feature` checked out.
fn commit_on_main(repo: &TestRepo, message: &str, files: &[(&str, &str)]) {
    repo.checkout("main");
    repo.commit_files(message, files);
    repo.checkout("feature");
}

#[test]
fn f8_approving_squashes_the_item_onto_the_target_and_tells_its_agent() {
    let (env, repo) = submitted();
    let base = repo.rev("main");
    let reply = approve(&env, 1, "Ship it.");
    assert_eq!(reply.status, 200, "{}", reply.body);
    let merge = json_of(&reply);
    let commit = repo.rev("main");
    assert_eq!(
        merge["outcome"],
        json!({ "kind": "merged", "commit": commit })
    );
    assert_eq!(merge["item"]["state"], "merged");

    let show = repo.git(["show", "-s", "--format=%an <%ae>|%s|%P", "main"]);
    assert_eq!(
        show.trim(),
        format!("Operator <operator@example.com>|Add login (app#1)|{base}")
    );
    assert_eq!(repo.rev("main^{tree}"), repo.rev("feature^{tree}"));

    // The merged item still shows its diff, read from the merge.
    let shown = item(&env);
    assert_eq!(shown["item"]["state"], "merged");
    assert_eq!(shown["blocked"], Value::Null);
    assert_eq!(shown["files"][0]["path"], "src/login.rs");

    let wait = env.mg(&["wait", "app#1"]).assert_code(3);
    assert_eq!(
        wait.stdout.trim(),
        format!(
            "[1] app#1 r1: the operator approved\n  Ship it.\n\n\
             [2] app#1 r1: the operator merged it as {commit}\n\n  \
             Clean up feature as agreed with the operator; by default:\n    \
             git switch --detach\n    git branch -D feature\n\n\
             cursor 2 (mg wait --since 2 replays from here)\n\napp#1 is merged"
        )
    );
    // The squash leaves the branch unmerged to git, so the event names it.
    let replay = env
        .mg(&["--json", "feedback", "app#1", "--since", "1"])
        .assert_code(0)
        .json();
    assert_eq!(replay["events"][0]["type"], "item.merged");
    assert_eq!(replay["events"][0]["branch"], "feature");
    let inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    assert_eq!(
        inbox,
        json!({ "needs_you": [], "with_agents": [], "merging": [], "rules": [] })
    );
}

#[test]
fn f9_a_conflict_sends_the_item_back_to_rebase() {
    let (env, repo) = submitted();
    commit_on_main(
        &repo,
        "Login too",
        &[("src/login.rs", "fn login(x: u8) {}\n")],
    );
    let target = repo.rev("main");
    let reply = approve(&env, 1, "");
    assert_eq!(reply.status, 200, "{}", reply.body);
    let merge = json_of(&reply);
    let conflict = json!({
        "kind": "conflict",
        "target_commit": target,
        "paths": ["src/login.rs"],
    });
    assert_eq!(merge["outcome"], conflict);
    assert_eq!(merge["item"]["state"], "changes_requested");
    assert_eq!(repo.rev("main"), target);
    assert_eq!(item(&env)["blocked"], conflict);
    let inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    assert_eq!(inbox["with_agents"][0]["blocked"], conflict);

    let feedback = env.mg(&["--json", "wait", "app#1"]).assert_code(0).json();
    assert_eq!(feedback["state"], "changes_requested");
    assert_eq!(feedback["events"][0]["verdict"], "approved");
    assert_eq!(
        feedback["events"][1],
        json!({
            "cursor": 2,
            "item": "app#1",
            "revision": 1,
            "source": "operator",
            "type": "merge.conflict",
            "target_commit": target,
            "paths": ["src/login.rs"],
            "commands": ["git fetch mergegate", "git rebase mergegate/main"],
        })
    );
    let text = env
        .mg(&["feedback", "app#1", "--since", "1"])
        .assert_code(0);
    assert_eq!(
        text.stdout.trim(),
        format!(
            "[2] app#1 r1: merging conflicts with the target at {}\n  src/login.rs\n\n  \
             Rebase and submit again:\n    git fetch mergegate\n    git rebase mergegate/main\n\n\
             cursor 2 (mg wait --since 2 replays from here)",
            &target[..12]
        )
    );

    // The agent rebases as told and submits; the new revision merges.
    repo.git(["fetch", "--quiet", "mergegate"]);
    repo.git(["rebase", "--quiet", "-X", "theirs", "mergegate/main"]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    assert_eq!(item(&env)["blocked"], Value::Null);
    let merged = json_of(&approve(&env, 2, ""));
    assert_eq!(merged["outcome"]["kind"], "merged");
    assert_eq!(repo.rev("main^"), target);
}

#[test]
fn f8_a_dirty_checkout_of_the_target_holds_the_merge_until_retried() {
    let (env, repo) = submitted();
    repo.checkout("main");
    std::fs::write(repo.root().join("README"), "edited\n").unwrap();
    let before = repo.rev("main");

    let merge = json_of(&approve(&env, 1, ""));
    assert_eq!(merge["item"]["state"], "merging");
    assert_eq!(merge["outcome"]["kind"], "held");
    let reason = merge["outcome"]["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("refused main"), "{reason}");
    assert_eq!(repo.rev("main"), before);
    assert_eq!(
        item(&env)["blocked"],
        json!({ "kind": "held", "reason": reason })
    );
    let inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    assert_eq!(inbox["merging"][0]["blocked"]["kind"], "held");

    // Held is still the operator's to act on, so the agent's wait for them
    // keeps waiting; a plain wait shows it.
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    assert_eq!(feedback["events"][1]["type"], "merge.held");

    std::fs::write(repo.root().join("README"), "title\n").unwrap();
    let retried = send(&env, "POST", "/api/items/app%231/merge", &json!({}));
    assert_eq!(retried.status, 200, "{}", retried.body);
    let retried = json_of(&retried);
    assert_eq!(retried["outcome"]["kind"], "merged");
    assert_eq!(repo.rev("main^"), before);
    assert_eq!(
        std::fs::read_to_string(repo.root().join("src/login.rs")).unwrap(),
        "fn login() {}\n",
        "the checkout updated in place"
    );
}

#[test]
fn f8_cancelling_a_held_merge_puts_the_item_back_in_review() {
    let (env, repo) = submitted();
    repo.checkout("main");
    std::fs::write(repo.root().join("README"), "edited\n").unwrap();
    assert_eq!(json_of(&approve(&env, 1, ""))["outcome"]["kind"], "held");

    let reply = send(&env, "DELETE", "/api/items/app%231/merge", &json!({}));
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(json_of(&reply)["state"], "needs_review");
    assert_eq!(item(&env)["blocked"], Value::Null);
    let text = env
        .mg(&["feedback", "app#1", "--since", "2"])
        .assert_code(0);
    assert_eq!(
        text.stdout.trim(),
        "[3] app#1 r1: the operator cancelled the held merge; it needs review again\n\n\
         cursor 3 (mg wait --since 3 replays from here)"
    );

    let again = send(&env, "DELETE", "/api/items/app%231/merge", &json!({}));
    assert_eq!(again.status, 409, "{}", again.body);
    assert_eq!(json_of(&again)["error"]["code"], "not_merging");
}

#[test]
fn f8_only_the_newest_revision_of_an_open_item_is_approved() {
    let (env, repo) = submitted();
    repo.commit_files("More", &[("src/more.rs", "fn more() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    let stale = approve(&env, 1, "");
    assert_eq!(stale.status, 409, "{}", stale.body);
    assert_eq!(json_of(&stale)["error"]["code"], "stale_revision");

    // The reviews route doesn't merge.
    let body = json!({ "revision": 2, "verdict": "approved" });
    let review = send(&env, "POST", "/api/items/app%231/reviews", &body);
    assert_eq!(review.status, 400, "{}", review.body);
    assert_eq!(json_of(&review)["error"]["code"], "invalid_verdict");

    assert_eq!(json_of(&approve(&env, 2, ""))["item"]["state"], "merged");
    let closed = approve(&env, 2, "");
    assert_eq!(closed.status, 409, "{}", closed.body);
    assert_eq!(json_of(&closed)["error"]["code"], "item_closed");
}
