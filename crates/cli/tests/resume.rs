//! An agent session ended mid-review, and `mg resume` brings a new one
//! up to date without taking the old one's unread events from `mg wait`.

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

fn comment(env: &Env, line: u32, body: &str) {
    let draft = json!({
        "revision": 1, "path": "src/login.rs", "side": "new",
        "start_line": line, "line": line, "body": body,
    });
    post(env, "/api/items/app%231/drafts", &draft);
}

/// `app#1`, reviewed with c_1 and c_2; its agent rejected c_1.
fn reviewed() -> (Env, TestRepo) {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files(
        "Add login",
        &[("src/login.rs", "fn login() {}\nfn logout() {}\n")],
    );
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    comment(&env, 1, "Rename it to sign_in.");
    comment(&env, 2, "Clear the token here.");
    post(
        &env,
        "/api/items/app%231/reviews",
        &json!({ "revision": 1, "verdict": "changes_requested", "summary": "Close.\nTwo things." }),
    );
    env.mg(&[
        "reply",
        "c_1",
        "--status",
        "rejected",
        "--body",
        "login is the public name.",
    ])
    .assert_code(0);
    (env, repo)
}

#[test]
fn f14_resume_prints_the_item_its_last_review_and_the_open_comments() {
    let (env, repo) = reviewed();
    let worktree = repo.root().canonicalize().unwrap();
    let run = env.mg(&["resume", "app#1"]).assert_code(0);
    let (head, skill) = run
        .stdout
        .split_once("\n--- How to work with mergegate ---\n\n")
        .unwrap();
    assert_eq!(
        head,
        format!(
            "You are picking up mergegate item app#1, \"Add login\", from an agent session that \
             ended.\n\n\
             - State: changes requested, at revision r1\n\
             - Branch: feature, onto main\n\
             - Worktree: {} (work there)\n\n\
             The operator's last review, on r1: changes requested\n  Close.\n  Two things.\n\n\
             Comments still waiting on you, oldest first:\n\n  \
             c_2 src/login.rs:2\n    > fn logout() {{}}\n    Clear the token here.\n\n\
             Next: run `mg feedback app#1` for the events the last session didn't read, reply \
             to every comment above with `mg reply`, `mg submit` the changes from the worktree, \
             then wait with `mg wait app#1`.\n",
            worktree.display()
        )
    );
    assert!(skill.starts_with("# mergegate\n"), "{skill}");
    // The references for working on an item come with it; the rest are only
    // named, as every pickup pays for them.
    for path in ["references/authoring.md", "references/waiting.md"] {
        assert!(skill.contains(&format!("\n--- {path} ---\n")), "{skill}");
    }
    assert!(
        !skill.contains("\n--- references/setup.md ---\n"),
        "{skill}"
    );
    assert!(
        skill.trim_end().ends_with(
            "\n--- Left out here: references/setup.md, references/operator.md; \
             `mg skill install` writes the whole skill ---"
        ),
        "{skill}"
    );
}

#[test]
fn f14_resume_names_the_wake_the_old_session_left() {
    let (env, repo) = reviewed();
    env.mg_at(repo.root(), &["submit", "--wake", "harness resume old"])
        .assert_code(0);
    let run = env.mg(&["resume", "app#1"]).assert_code(0);
    assert!(
        run.stdout.contains(
            "- Wake: `harness resume old`, left by the session that ended. It would start that \
             session, not you: replace it with `mg submit --wake CMD` from the worktree, or \
             remove it with `mg submit --wake \"\"`.\n"
        ),
        "{}",
        run.stdout
    );
}

#[test]
fn f14_a_closed_item_leaves_nothing_to_answer() {
    let (env, repo) = reviewed();
    env.mg_at(repo.root(), &["submit", "--wake", "harness resume old"])
        .assert_code(0);
    env.mg(&["abandon", "app#1"]).assert_code(0);
    let run = env.mg(&["resume", "app#1"]).assert_code(0);
    let (head, _) = run
        .stdout
        .split_once("\n--- How to work with mergegate ---\n\n")
        .unwrap();
    assert!(!head.contains("Wake:"), "{head}");
    assert!(!head.contains("c_2"), "{head}");
    let worktree = repo.root().canonicalize().unwrap();
    let line = format!(
        "- Worktree: {} (submitted from there)\n",
        worktree.display()
    );
    assert!(head.contains(&line), "{head}");
    // The replay repeats the cleanup, which may have run, and the branch
    // name may be newer work's by now.
    let next = format!(
        "The item is closed. Run `mg feedback app#1 --since 0` to read how it ended. Clean up \
         as agreed with the operator only if feature still points at r1's head, {}: otherwise \
         it is done, or the name is newer work's.\n",
        &repo.rev("feature")[..12]
    );
    assert!(head.ends_with(&next), "{head}");

    // A wait that already delivered the end leaves the replay to show it.
    env.mg(&["wait", "app#1"]).assert_code(0);
    let replay = env
        .mg(&["feedback", "app#1", "--since", "0"])
        .assert_code(0);
    assert!(
        replay
            .stdout
            .contains("item.abandoned app#1 r1: its agent abandoned it"),
        "{}",
        replay.stdout
    );
}

#[test]
fn f14_resume_leaves_the_events_for_mg_wait() {
    let (env, _repo) = reviewed();
    let resumed = env.mg(&["--json", "resume", "app#1"]).assert_code(0).json();
    assert_eq!(resumed["item"]["id"], "app#1");
    assert_eq!(resumed["review"]["verdict"], "changes_requested");
    let open: Vec<_> = resumed["open"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["comment"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(open, ["c_2"]);
    assert!(resumed["prompt"].as_str().unwrap().contains("c_2"));

    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    assert_eq!(feedback["events"][0]["type"], "review.submitted");
}

#[test]
fn f14_a_question_answered_is_still_open() {
    let (env, _repo) = reviewed();
    env.mg(&[
        "reply",
        "c_2",
        "--status",
        "question",
        "--body",
        "Which token?\nThere are two.",
    ])
    .assert_code(0);
    let resumed = env.mg(&["resume", "app#1"]).assert_code(0);
    assert!(
        resumed.stdout.contains(
            "    Clear the token here.\n    you (question): Which token?\n      There are two.\n"
        ),
        "{}",
        resumed.stdout
    );
}

#[test]
fn f14_resume_names_an_unknown_item() {
    let (env, _repo) = reviewed();
    let run = env.mg(&["--json", "resume", "app#9"]).assert_code(1);
    assert_eq!(run.json()["error"]["code"], "item_not_found");
}
