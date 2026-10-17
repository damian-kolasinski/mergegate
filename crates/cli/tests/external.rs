//! External review, against a fake GitHub: comments on an item's pull
//! request reach its agent in one batch once reviewers go quiet (F5),
//! expected bots hold the batch until they've commented, `mg sync` hands
//! over what's held at once, and the operator agrees with or overrides the
//! agent's rejection of one.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use testkit::{Env, FakeGithub, GithubNewComment, Reply, TestRepo, http_reply};

const ACME: &str = "acme/app";
/// Long enough that nothing in a test waits it out.
const NEVER: &str = "600000";

/// A hub with `app` on the github backend and `app#1`, pull request 1,
/// adding `src/login.rs` on `login`; `timing` is the poll interval, the
/// quiet window and the cap, in milliseconds.
fn submitted(github: &FakeGithub, timing: [&str; 3]) -> (Env, TestRepo) {
    let mut vars = github.env();
    for (name, value) in [
        "MG_GITHUB_POLL_MS",
        "MG_EXTERNAL_QUIET_MS",
        "MG_EXTERNAL_CAP_MS",
    ]
    .into_iter()
    .zip(timing)
    {
        vars.push((name.to_owned(), value.to_owned()));
    }
    let env = Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(vars);
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    repo.git(["remote", "add", "origin", "git@github.com:acme/app.git"]);
    github.create_repo(ACME, repo.root());
    env.mg_at(
        repo.root(),
        &["--json", "project", "add", "--backend", "github"],
    )
    .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "login"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    (env, repo)
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

/// The one comment in `comments` whose body is `body`.
fn with_body<'a>(comments: &'a Value, body: &str) -> &'a Value {
    let found = comments
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["body"] == body);
    found.unwrap_or_else(|| panic!("no {body:?} in {comments}"))
}

fn id(comment: &Value) -> &str {
    comment["id"].as_str().unwrap()
}

#[test]
fn f5_comments_reach_the_agent_in_one_batch_once_reviewers_go_quiet() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github, ["100", "500", NEVER]);
    let inline = GithubNewComment::inline("codex[bot]", "Hash it.", "src/login.rs", None, 1);
    github.comment(ACME, 1, inline);
    github.comment(ACME, 1, GithubNewComment::issue("alice", "Needs a test."));
    github.comment(ACME, 1, GithubNewComment::review("alice", "Close."));
    // The operator's own comments are theirs to make in mergegate.
    github.comment(ACME, 1, GithubNewComment::issue("operator", "Mine."));

    let feedback = env
        .mg(&["--json", "wait", "app#1", "--timeout", "30"])
        .assert_code(0)
        .json();
    let events = feedback["events"].as_array().unwrap();
    assert_eq!(events.len(), 1, "{feedback}");
    let event = &events[0];
    assert_eq!(
        (&event["type"], &event["source"], &event["edited"]),
        (&json!("external.comments"), &json!("external"), &json!([]))
    );
    let comments = &event["comments"];
    assert_eq!(comments.as_array().unwrap().len(), 3, "{comments}");
    let hash = with_body(comments, "Hash it.");
    assert_eq!(
        (&hash["path"], &hash["line"], &hash["quote"]),
        (&json!("src/login.rs"), &json!(1), &json!("fn login() {}"))
    );
    assert_eq!(hash["external"]["author"], "codex[bot]");
    assert_eq!(hash["external"]["bot"], true);
    assert_eq!(hash["external"]["kind"], "review_comment");
    let test = with_body(comments, "Needs a test.");
    assert_eq!((&test["path"], &test["line"]), (&Value::Null, &Value::Null));
    assert_eq!(test["external"]["kind"], "issue_comment");
    assert_eq!(with_body(comments, "Close.")["external"]["kind"], "review");

    let text = env
        .mg(&["feedback", "app#1", "--since", "0"])
        .assert_code(0);
    for line in [
        "reviewers commented on its pull request",
        &format!(
            "{} src/login.rs:1, from codex[bot] (bot) on GitHub",
            id(hash)
        ),
        "    > fn login() {}",
        &format!("{} the pull request, from alice on GitHub", id(test)),
    ] {
        assert!(
            text.stdout.contains(line),
            "no {line:?} in\n{}",
            text.stdout
        );
    }
}

#[test]
fn f5_expected_bots_hold_the_batch_until_they_have_commented() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github, ["100", "200", NEVER]);
    env.mg(&["--json", "project", "set", "app", "bots", "codex"])
        .assert_code(0);
    github.comment(ACME, 1, GithubNewComment::issue("alice", "Needs a test."));
    env.mg(&["--json", "wait", "app#1", "--timeout", "2"])
        .assert_code(75);

    github.comment(ACME, 1, GithubNewComment::issue("codex[bot]", "LGTM."));
    let feedback = env
        .mg(&["--json", "wait", "app#1", "--timeout", "30"])
        .assert_code(0)
        .json();
    let comments = &feedback["events"][0]["comments"];
    assert_eq!(comments.as_array().unwrap().len(), 2, "{feedback}");
}

#[test]
fn sync_hands_over_held_comments_at_once_and_edits_again() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github, [NEVER, NEVER, NEVER]);
    let posted = github.comment(ACME, 1, GithubNewComment::issue("alice", "Needs a test."));

    let synced = env.mg(&["--json", "sync", "app#1"]).assert_code(0).json();
    assert_eq!(synced, json!({ "new": 1, "edited": 0, "released": 1 }));
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    let comment = &feedback["events"][0]["comments"][0];
    assert_eq!(comment["body"], "Needs a test.", "{feedback}");

    github.edit_comment(posted, "Needs two tests.");
    let synced = env.mg(&["sync", "app#1"]).assert_code(0);
    assert_eq!(
        synced.stdout.trim(),
        "synced app#1 with GitHub: 0 new, 1 edited\n\
         1 comment is waiting for you: mg feedback app#1"
    );
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    let event = &feedback["events"][0];
    assert_eq!(event["comments"][0]["body"], "Needs two tests.");
    assert_eq!(event["edited"], json!([comment["id"]]));

    let synced = env.mg(&["--json", "sync", "app#1"]).assert_code(0).json();
    assert_eq!(synced, json!({ "new": 0, "edited": 0, "released": 0 }));
}

#[test]
fn the_operator_agrees_with_or_overrides_a_rejection() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github, [NEVER, NEVER, NEVER]);
    github.comment(ACME, 1, GithubNewComment::issue("alice", "Rename it."));
    github.comment(ACME, 1, GithubNewComment::issue("alice", "Split it."));
    env.mg(&["--json", "sync", "app#1"]).assert_code(0);
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    let comments = &feedback["events"][0]["comments"];
    let rename = id(with_body(comments, "Rename it.")).to_owned();
    let split = id(with_body(comments, "Split it.")).to_owned();

    // Nothing to agree with before the agent rejects it.
    let early = post(&env, &format!("/api/comments/{rename}/agree"), &json!({}));
    assert_eq!(early.status, 409, "{}", early.body);
    for c in [&rename, &split] {
        env.mg(&[
            "--json", "reply", c, "--status", "rejected", "--body", "No.",
        ])
        .assert_code(0);
    }

    let agreed = post(&env, &format!("/api/comments/{split}/agree"), &json!({}));
    assert_eq!(agreed.status, 200, "{}", agreed.body);
    assert!(json_of(&agreed)["external"]["agreed_reply"].is_number());
    let overridden = post(
        &env,
        &format!("/api/comments/{rename}/override"),
        &json!({ "note": "The old name misleads." }),
    );
    assert_eq!(overridden.status, 201, "{}", overridden.body);
    assert_eq!(json_of(&overridden)["overrides"], true);

    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    let events = feedback["events"].as_array().unwrap();
    assert_eq!(events.len(), 1, "agreeing sends nothing: {feedback}");
    assert_eq!(
        (&events[0]["type"], &events[0]["source"]),
        (&json!("comment.override"), &json!("operator"))
    );
    assert_eq!(events[0]["comment"]["id"], rename);
    assert_eq!(events[0]["reply"]["body"], "The old name misleads.");
    let text = env
        .mg(&["feedback", "app#1", "--since", "0"])
        .assert_code(0);
    assert!(
        text.stdout.contains(&format!(
            "the operator overrides your rejection of {rename}"
        )),
        "{}",
        text.stdout
    );

    // The rejection is no longer the latest reply: it's overridden once.
    let again = post(
        &env,
        &format!("/api/comments/{rename}/override"),
        &json!({}),
    );
    assert_eq!(again.status, 409, "{}", again.body);
    let resumed = env.mg(&["resume", "app#1"]).assert_code(0);
    assert!(
        resumed.stdout.contains("operator (overrides you)"),
        "{}",
        resumed.stdout
    );
}

#[test]
fn f5_a_batch_wakes_the_agent() {
    let github = FakeGithub::start();
    let (env, repo) = submitted(&github, ["100", "200", NEVER]);
    let woken = env.home().join("woken");
    let wake = format!("printf '%s' \"$MG_ITEM\" > {}", woken.display());
    env.mg_at(repo.root(), &["--json", "submit", "--wake", &wake])
        .assert_code(0);
    github.comment(ACME, 1, GithubNewComment::issue("alice", "Needs a test."));

    let deadline = Instant::now() + Duration::from_secs(10);
    while !woken.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(std::fs::read_to_string(&woken).unwrap_or_default(), "app#1");
}

fn conversation(env: &Env) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!(
            "GET /api/items/app%231/conversation HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
             {}: ui\r\n\r\n",
            api::CLIENT_HEADER,
        ),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    json_of(&reply)
}

#[test]
fn a_wait_for_the_operator_leaves_held_comments_held() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github, ["100", NEVER, NEVER]);
    github.comment(ACME, 1, GithubNewComment::issue("alice", "Rename it."));
    env.mg(&["--json", "sync", "app#1"]).assert_code(0);
    github.comment(ACME, 1, GithubNewComment::issue("alice", "Split it."));
    let deadline = Instant::now() + Duration::from_secs(30);
    while conversation(&env)["comments"].as_array().unwrap().len() < 2 {
        assert!(Instant::now() < deadline, "the poller never saw it");
        std::thread::sleep(Duration::from_millis(50));
    }

    // The batch it doesn't wait for stays unacknowledged, and the comment
    // after it waits for its own batch rather than riding along.
    env.mg(&[
        "--json",
        "wait",
        "app#1",
        "--for",
        "operator",
        "--timeout",
        "1",
    ])
    .assert_code(75);
    let comments = conversation(&env)["comments"].clone();
    assert_eq!(
        with_body(&comments, "Split it.")["external"]["pending"],
        true
    );
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    let events = feedback["events"].as_array().unwrap();
    assert_eq!(
        events.len(),
        2,
        "a read the agent gets takes it: {feedback}"
    );
}

#[test]
fn a_wait_since_the_printed_cursor_takes_held_comments_along() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github, ["100", NEVER, NEVER]);
    github.comment(ACME, 1, GithubNewComment::issue("alice", "Rename it."));
    env.mg(&["--json", "sync", "app#1"]).assert_code(0);
    let first = env.mg(&["--json", "wait", "app#1"]).assert_code(0).json();
    let cursor = first["cursor"].to_string();
    github.comment(ACME, 1, GithubNewComment::issue("alice", "Split it."));
    until(&env, "the poller never saw it", |c| {
        c.as_array().unwrap().len() == 2
    });

    // A look back before the delivered cursor leaves it held.
    let back = env
        .mg(&["--json", "feedback", "app#1", "--since", "0"])
        .assert_code(0)
        .json();
    assert_eq!(back["events"].as_array().unwrap().len(), 1, "{back}");
    let comments = conversation(&env)["comments"].clone();
    assert_eq!(
        with_body(&comments, "Split it.")["external"]["pending"],
        true
    );

    // Held comments alone don't wake a wait; the review does, and takes
    // them along.
    let review = json!({ "revision": 1, "verdict": "comment", "summary": "Looks close." });
    let reply = post(&env, "/api/items/app%231/reviews", &review);
    assert_eq!(reply.status, 201, "{}", reply.body);
    let next = env
        .mg(&["--json", "wait", "app#1", "--since", &cursor])
        .assert_code(0)
        .json();
    let events = next["events"].as_array().unwrap();
    let types: Vec<_> = events.iter().map(|e| e["type"].clone()).collect();
    assert_eq!(
        types,
        [json!("review.submitted"), json!("external.comments")],
        "{next}"
    );
    assert_eq!(events[1]["comments"][0]["body"], "Split it.", "{next}");
    let again = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    assert_eq!(again["events"], json!([]), "{again}");
}

/// Polls the item's conversation until `done` holds for its comments.
fn until(env: &Env, what: &str, done: impl Fn(&Value) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !done(&conversation(env)["comments"]) {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_comment_deleted_on_github_is_dropped_unless_a_batch_carried_it() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github, ["100", NEVER, NEVER]);
    let doomed = github.comment(ACME, 1, GithubNewComment::issue("alice", "Typo here."));
    let kept = github.comment(ACME, 1, GithubNewComment::issue("alice", "Needs a test."));
    until(&env, "the poller never saw them", |c| {
        c.as_array().unwrap().len() == 2
    });

    github.delete_comment(doomed);
    let synced = env.mg(&["--json", "sync", "app#1"]).assert_code(0).json();
    assert_eq!(synced, json!({ "new": 0, "edited": 0, "released": 1 }));
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    let bodies: Vec<_> = feedback["events"][0]["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["body"].clone())
        .collect();
    assert_eq!(bodies, [json!("Needs a test.")], "{feedback}");
    let comments = conversation(&env)["comments"].clone();
    assert_eq!(comments.as_array().unwrap().len(), 1, "{comments}");

    // One a batch carried stays, as the agent got it, with the operator's
    // draft reply: its held edit is withdrawn with it.
    let comment = id(&comments[0]).to_owned();
    let drafted = post(
        &env,
        &format!("/api/comments/{comment}/drafts"),
        &json!({ "body": "Which test?" }),
    );
    assert_eq!(drafted.status, 201, "{}", drafted.body);
    github.edit_comment(kept, "Needs two tests.");
    until(&env, "the poller never saw the edit", |c| {
        c[0]["body"] == "Needs two tests."
    });
    github.delete_comment(kept);
    let synced = env.mg(&["--json", "sync", "app#1"]).assert_code(0).json();
    assert_eq!(synced, json!({ "new": 0, "edited": 0, "released": 0 }));
    let shown = conversation(&env);
    let comments = &shown["comments"];
    assert_eq!(comments.as_array().unwrap().len(), 1, "{comments}");
    assert_eq!(comments[0]["body"], "Needs a test.");
    assert_eq!(comments[0]["external"]["pending"], false);
    assert_eq!(shown["draft_replies"][0]["body"], "Which test?", "{shown}");
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    assert_eq!(feedback["events"], json!([]), "{feedback}");
}

#[test]
fn f5_an_expected_bots_comment_from_before_the_newest_push_doesnt_count() {
    let github = FakeGithub::start();
    let (env, repo) = submitted(&github, [NEVER, "200", NEVER]);
    env.mg(&["--json", "project", "set", "app", "bots", "codex"])
        .assert_code(0);
    github.comment(
        ACME,
        1,
        GithubNewComment::issue("codex[bot]", "r1 looks fine."),
    );
    github.comment(ACME, 1, GithubNewComment::issue("alice", "Needs a test."));
    repo.commit_files("Add a test", &[("src/login_test.rs", "\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    // The hub first sees both comments after the push.
    env.mg(&["hub", "stop"]).assert_code(0);
    env.mg_with_env(
        &["--json", "hub", "start", "--port", "0"],
        &[("MG_GITHUB_POLL_MS", "100")],
    )
    .assert_code(0);
    until(&env, "the poller never saw them", |c| {
        c.as_array().unwrap().len() == 2
    });
    env.mg(&["--json", "wait", "app#1", "--timeout", "2"])
        .assert_code(75);

    github.comment(
        ACME,
        1,
        GithubNewComment::issue("codex[bot]", "r2 looks fine."),
    );
    let feedback = env
        .mg(&["--json", "wait", "app#1", "--timeout", "30"])
        .assert_code(0)
        .json();
    let comments = &feedback["events"][0]["comments"];
    assert_eq!(comments.as_array().unwrap().len(), 3, "{feedback}");
}

#[test]
fn an_expected_bot_editing_its_comment_after_the_newest_push_counts() {
    let github = FakeGithub::start();
    let (env, repo) = submitted(&github, [NEVER, "200", NEVER]);
    env.mg(&["--json", "project", "set", "app", "bots", "codex"])
        .assert_code(0);
    let summary = github.comment(
        ACME,
        1,
        GithubNewComment::issue("codex[bot]", "Reviewing r1."),
    );
    repo.commit_files("Add a test", &[("src/login_test.rs", "\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    env.mg(&["hub", "stop"]).assert_code(0);
    env.mg_with_env(
        &["--json", "hub", "start", "--port", "0"],
        &[("MG_GITHUB_POLL_MS", "100")],
    )
    .assert_code(0);
    until(&env, "the poller never saw it", |c| {
        c.as_array().unwrap().len() == 1
    });
    // Also takes the edit below past the push's second, the finest date
    // GitHub gives.
    env.mg(&["--json", "wait", "app#1", "--timeout", "2"])
        .assert_code(75);

    // A bot that keeps one summary comment edits it when it reviews a push.
    github.edit_comment(summary, "r2 looks fine.");
    let feedback = env
        .mg(&["--json", "wait", "app#1", "--timeout", "30"])
        .assert_code(0)
        .json();
    let comments = &feedback["events"][0]["comments"];
    assert_eq!(comments.as_array().unwrap().len(), 1, "{feedback}");
    assert_eq!(comments[0]["body"], "r2 looks fine.", "{feedback}");
}

/// Waits out `polls` more reads of the pull request's conversation.
fn polled(github: &FakeGithub, polls: usize) {
    let reads = || {
        let list = "GET /api/repos/acme/app/issues/1/comments";
        github.requests().iter().filter(|r| *r == list).count()
    };
    let until = reads() + polls;
    let deadline = Instant::now() + Duration::from_secs(30);
    while reads() < until {
        assert!(Instant::now() < deadline, "the poller stopped");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_held_comment_a_list_misses_stays_until_github_says_it_is_gone() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github, ["100", NEVER, NEVER]);
    let missed = github.comment(ACME, 1, GithubNewComment::issue("alice", "Typo here."));
    until(&env, "the poller never saw it", |c| {
        c.as_array().unwrap().len() == 1
    });
    let comment = id(&conversation(&env)["comments"][0]).to_owned();
    let drafted = post(
        &env,
        &format!("/api/comments/{comment}/drafts"),
        &json!({ "body": "Which one?" }),
    );
    assert_eq!(drafted.status, 201, "{}", drafted.body);

    // A list read across a deletion skips it, but GitHub still has it.
    github.unlist_comment(missed);
    polled(&github, 3);
    let shown = conversation(&env);
    assert_eq!(shown["comments"][0]["id"], comment.as_str(), "{shown}");
    assert_eq!(shown["comments"][0]["external"]["pending"], true);
    assert_eq!(shown["draft_replies"][0]["body"], "Which one?", "{shown}");
    let asked = format!("GET /api/repos/acme/app/issues/comments/{missed}");
    assert!(github.requests().contains(&asked));

    github.delete_comment(missed);
    until(&env, "the poller never withdrew it", |c| {
        c.as_array().unwrap().is_empty()
    });
    assert_eq!(conversation(&env)["draft_replies"], json!([]));
}

#[test]
fn the_poller_closes_an_item_merged_on_github_and_wakes_its_agent() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github, ["100", NEVER, NEVER]);
    github.comment(
        ACME,
        1,
        GithubNewComment::issue("alice", "Merging it myself."),
    );
    let commit = github.merge_on_github(ACME, 1);

    // No sync: the poller alone closes it, which ends the agent's wait. The
    // comment, though reviewers never went quiet, goes first.
    let wait = env.mg(&["wait", "app#1", "--timeout", "20"]);
    assert_eq!(wait.code, 0, "{}{}", wait.stdout, wait.stderr);
    let said = wait.stdout.find("Merging it myself.");
    let merged = wait.stdout.find("it was merged on GitHub");
    assert!(said.is_some() && said < merged, "{}", wait.stdout);
    assert!(
        wait.stdout
            .contains(&format!("it was merged on GitHub as {commit}")),
        "{}",
        wait.stdout
    );
    let feedback = env
        .mg(&["--json", "feedback", "app#1", "--since", "0"])
        .assert_code(0)
        .json();
    assert_eq!(feedback["state"], "merged", "{feedback}");
}
