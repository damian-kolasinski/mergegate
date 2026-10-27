//! The github backend, against a fake GitHub: registering a repo with a
//! GitHub remote, submitting opens and updates a pull request,
//! approving squash-merges it there, abandoning closes it, and a
//! project switches backends under open items.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};
use testkit::{Env, FakeGithub, GithubPull, GithubPullState, Reply, TestRepo, http_reply};

const ACME: &str = "acme/app";

fn env(github: &FakeGithub) -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(github.env())
}

fn error_code(run: &Value) -> &str {
    run["error"]["code"].as_str().unwrap_or_default()
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

fn approve(env: &Env, revision: u32) -> Value {
    let body = json!({ "revision": revision, "summary": "" });
    let reply = send(env, "POST", "/api/items/app%231/approve", &body);
    assert_eq!(reply.status, 200, "{}", reply.body);
    json_of(&reply)
}

/// The operator's `app`, with `origin` on GitHub as `acme/app`, which holds
/// the same history.
fn on_github(github: &FakeGithub) -> TestRepo {
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    repo.git(["remote", "add", "origin", "git@github.com:acme/app.git"]);
    github.create_repo(ACME, repo.root());
    repo
}

/// Pushes the operator's `branch` to GitHub, as someone working there would.
fn push_to_github(github: &FakeGithub, repo: &TestRepo, branch: &str) {
    let (url, header) = github.remote(ACME);
    repo.git([
        "-c",
        &format!("http.extraheader={header}"),
        "push",
        "--quiet",
        "--force",
        &url,
        &format!("{branch}:{branch}"),
    ]);
}

/// A hub with `app` on the github backend, its repo's `mergegate` remote
/// added, and `app#1` adding `src/login.rs` on `login` submitted.
fn submitted(github: &FakeGithub) -> (Env, TestRepo) {
    let env = env(github);
    env.start_hub();
    let repo = on_github(github);
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

/// `GET /api/items/app%231`: the item with its files.
fn item(env: &Env) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!("GET /api/items/app%231 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    json_of(&reply)["item"].clone()
}

/// `mg doctor`'s check of `app`.
fn doctor_app(doctor: &Value) -> Value {
    let checks = doctor["checks"].as_array().unwrap();
    let app = checks.iter().find(|c| c["name"] == "project app");
    app.unwrap_or_else(|| panic!("{doctor}")).clone()
}

fn pull(github: &FakeGithub) -> GithubPull {
    let pulls = github.pulls();
    assert_eq!(pulls.len(), 1, "{pulls:?}");
    pulls[0].clone()
}

#[test]
fn f1_registers_a_repo_with_a_github_remote_on_the_backend_chosen() {
    let github = FakeGithub::start();
    let env = env(&github);
    env.start_hub();
    let repo = on_github(&github);

    let asked = env
        .mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(2)
        .json();
    let decision = &asked["needs_decision"];
    assert_eq!(decision["flag"], "--backend");
    assert_eq!(decision["default"], "github");
    assert_eq!(decision["options"][0]["value"], "github");
    assert_eq!(decision["options"][1]["value"], "local");
    assert!(
        decision["question"]
            .as_str()
            .unwrap()
            .contains("pull requests on acme/app"),
        "{decision}"
    );

    let added = env
        .mg_at(
            repo.root(),
            &["--json", "project", "add", "--backend", "github"],
        )
        .assert_code(0)
        .json();
    assert_eq!(added["backend"], "github");
    assert_eq!(added["github"], ACME);
    let shown = env.mg(&["project", "show", "app"]).assert_code(0).stdout;
    assert!(shown.contains("  github    acme/app"), "{shown}");
    let doctor = env.mg(&["--json", "doctor"]).assert_code(0).json();
    assert_eq!(
        doctor_app(&doctor)["message"],
        format!("github acme/app · main · {}", repo.root().display())
    );
}

#[test]
fn f1_names_the_github_repo_when_no_remote_does() {
    let github = FakeGithub::start();
    let env = env(&github);
    env.start_hub();
    let repo = TestRepo::new("app");
    github.create_repo("me/app", repo.root());

    // A repo without a GitHub remote isn't asked about, and lands locally.
    let plain = TestRepo::new("plain");
    let added = env
        .mg_at(plain.root(), &["--json", "project", "add"])
        .assert_code(0)
        .json();
    assert_eq!(added["backend"], "local");

    let denied = TestRepo::new("denied");
    github.create_repo("me/denied", denied.root());
    github.deny_push("me/denied");
    let refused = env
        .mg_at(
            denied.root(),
            &["--json", "project", "add", "--github", "me/denied"],
        )
        .assert_code(1)
        .json();
    assert_eq!(error_code(&refused), "github_no_access");

    let added = env
        .mg_at(
            repo.root(),
            &["--json", "project", "add", "--github", "me/app"],
        )
        .assert_code(0)
        .json();
    assert_eq!(
        (&added["backend"], &added["github"]),
        (&json!("github"), &json!("me/app"))
    );
}

#[test]
fn f4_submitting_opens_a_pull_request_and_keeps_it_up_to_date() {
    let github = FakeGithub::start();
    let (env, repo) = submitted(&github);
    let url = format!("{}/acme/app/pull/1", github.web_url());
    assert_eq!(
        pull(&github),
        GithubPull {
            repo: ACME.into(),
            number: 1,
            branch: "login".into(),
            base: "main".into(),
            title: "Add login".into(),
            body: "Reviewed in mergegate as app#1.".into(),
            draft: false,
            state: GithubPullState::Open,
        }
    );
    assert_eq!(github.rev(ACME, "login"), Some(repo.rev("HEAD")));
    assert_eq!(
        item(&env)["link"],
        json!({ "label": "#1", "title": "Pull request on GitHub", "url": url })
    );
    let inbox = env.mg(&["inbox"]).assert_code(0).stdout;
    assert!(inbox.contains("login · #1"), "{inbox}");

    // A new revision force-pushes the branch; a new title retitles the PR,
    // and its guide describes it.
    repo.git(["commit", "--quiet", "--amend", "-m", "Add login and logout"]);
    let guide = repo.sibling("guide.md");
    std::fs::write(&guide, "Signs users in and out.\n").unwrap();
    let text = env
        .mg_at(
            repo.root(),
            &[
                "submit",
                "--title",
                "Add login and logout",
                "--guide",
                guide.to_str().unwrap(),
            ],
        )
        .assert_code(0)
        .stdout;
    assert!(text.contains(&format!("  #1 {url}")), "{text}");
    assert_eq!(github.rev(ACME, "login"), Some(repo.rev("HEAD")));
    let renamed = pull(&github);
    assert_eq!(
        (
            renamed.number,
            renamed.title.as_str(),
            renamed.body.as_str()
        ),
        (
            1,
            "Add login and logout",
            "Reviewed in mergegate as app#1.\n\nSigns users in and out."
        )
    );

    // Someone else's push to the branch on GitHub is never overwritten.
    repo.commit_files("Fix on GitHub", &[("src/fix.rs", "\n")]);
    push_to_github(&github, &repo, "login");
    let theirs = repo.rev("HEAD");
    repo.git(["reset", "--quiet", "--hard", "HEAD~1"]);
    repo.commit_files("Add logout", &[("src/logout.rs", "\n")]);
    let moved = env
        .mg_at(repo.root(), &["--json", "submit"])
        .assert_code(1)
        .json();
    assert_eq!(error_code(&moved), "branch_moved");
    assert_eq!(
        moved["error"]["message"],
        format!(
            "GitHub has login at {theirs}, which mergegate didn't push; build on it to submit again"
        )
    );
    assert_eq!(github.rev(ACME, "login"), Some(theirs.clone()));
    assert_eq!(item(&env)["revision"]["number"], 2);

    // Built on, it goes through.
    repo.git(["reset", "--quiet", "--hard", theirs.as_str()]);
    repo.commit_files("Add logout", &[("src/logout.rs", "\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    assert_eq!(github.rev(ACME, "login"), Some(repo.rev("HEAD")));
    assert_eq!(item(&env)["revision"]["number"], 3);
}

#[test]
fn f4_an_item_goes_by_its_pull_request() {
    let github = FakeGithub::start();
    github.open_elsewhere(ACME, 83);
    let (env, repo) = submitted(&github);
    assert_eq!(pull(&github).number, 84);
    let inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    let entry = &inbox["needs_you"][0];
    assert_eq!(
        (&entry["id"], &entry["name"]),
        (&json!("app#1"), &json!("app#84"))
    );
    let inbox = env.mg(&["inbox"]).assert_code(0).stdout;
    assert!(inbox.contains("  app#84 r1  Add login"), "{inbox}");

    repo.commit_files("Add logout", &[("src/logout.rs", "fn logout() {}\n")]);
    let text = env.mg_at(repo.root(), &["submit"]).assert_code(0).stdout;
    assert!(text.starts_with("submitted app#84 r2: "), "{text}");

    // Commands take its name, and its ID where no pull request has that
    // number.
    for name in ["app#84", "app#1"] {
        let feedback = env.mg(&["--json", "feedback", name]).assert_code(0).json();
        assert_eq!(
            (&feedback["item"], &feedback["name"]),
            (&json!("app#1"), &json!("app#84"))
        );
    }
    let missing = env
        .mg(&["--json", "feedback", "app#2"])
        .assert_code(1)
        .json();
    assert_eq!(error_code(&missing), "item_not_found");
    let text = env.mg(&["abandon", "app#84"]).assert_code(0).stdout;
    assert!(text.starts_with("abandoned app#84"), "{text}");
}

#[test]
fn f4_submitting_fails_while_github_is_out_of_reach() {
    let github = FakeGithub::start();
    let (env, repo) = submitted(&github);
    github.log_out();
    repo.commit_files("Add logout", &[("src/logout.rs", "\n")]);
    let failed = env
        .mg_at(repo.root(), &["--json", "submit"])
        .assert_code(1)
        .json();
    assert_eq!(error_code(&failed), "github_login");
    assert_eq!(failed["error"]["hint"], "gh auth login");
    assert_eq!(item(&env)["revision"]["number"], 1);

    let doctor = env.mg(&["--json", "doctor"]).assert_code(1).json();
    assert_eq!(doctor_app(&doctor)["status"], "fail");
    assert_eq!(doctor_app(&doctor)["fix"], "gh auth login");

    // Signing in again is all it takes.
    github.log_in("gho_second");
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    assert_eq!(github.rev(ACME, "login"), Some(repo.rev("HEAD")));
}

#[test]
fn f8_approving_squash_merges_the_pull_request_on_github() {
    let github = FakeGithub::start();
    let (env, repo) = submitted(&github);
    let head = repo.rev("HEAD");
    let base = repo.rev("main");
    let guide = repo.sibling("guide.md");
    std::fs::write(&guide, "Signs users in.\n\n## Files\n- `src/login.rs`\n").unwrap();
    env.mg_at(repo.root(), &["submit", "--guide", guide.to_str().unwrap()])
        .assert_code(0);

    let merge = approve(&env, 1);
    let commit = github.rev(ACME, "main").unwrap();
    assert_eq!(
        merge["outcome"],
        json!({ "kind": "merged", "commit": commit })
    );
    assert_eq!(merge["item"]["state"], "merged");
    assert_eq!(
        pull(&github).state,
        GithubPullState::Merged {
            head: head.clone(),
            commit: commit.clone()
        }
    );
    // The commit says what the change is for, without the guide's files.
    let show = github.git(ACME, ["show", "-s", "--format=%s|%b|%P", "main"]);
    assert_eq!(
        show.trim(),
        format!("Add login (app#1)|Signs users in.\n|{base}")
    );
    assert_eq!(github.rev(ACME, "login"), None);
    // The operator's own checkout is GitHub's to update, not the hub's.
    assert_eq!(repo.rev("main"), base);

    let wait = env.mg(&["wait", "app#1"]).assert_code(0).stdout;
    assert!(
        wait.contains(&format!(
            "item.merged app#1 r1: the operator merged it as {commit}"
        )),
        "{wait}"
    );
    // The default cleanup leaves the clone on the squash commit, though its
    // own main is behind.
    let replay = env
        .mg(&["--json", "feedback", "app#1", "--since", "0"])
        .assert_code(0)
        .json();
    let events = replay["events"].as_array().unwrap();
    let merged = events.iter().find(|e| e["type"] == "item.merged").unwrap();
    for command in merged["commands"].as_array().unwrap() {
        let args: Vec<_> = command.as_str().unwrap().split(' ').collect();
        repo.git(&args[1..]);
    }
    assert_eq!(repo.rev("HEAD"), commit);

    // The branch name is free for the next item, which pushes it again.
    repo.git(["switch", "--quiet", "-C", "login", "main"]);
    repo.commit_files("Add logout", &[("src/logout.rs", "\n")]);
    let next = env
        .mg_at(repo.root(), &["--json", "submit"])
        .assert_code(0)
        .json();
    assert_eq!(next["item"]["id"], "app#2");
    assert_eq!(github.rev(ACME, "login"), Some(repo.rev("HEAD")));
    assert_eq!(github.pulls().len(), 2);
}

#[test]
fn f8_a_merge_github_refuses_is_held_until_retried() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github);
    github.refuse_merges(Some("Required status check \"ci\" is expected."));

    let merge = approve(&env, 1);
    assert_eq!(merge["item"]["state"], "merging");
    assert_eq!(merge["outcome"]["kind"], "held");
    let reason = merge["outcome"]["reason"].as_str().unwrap();
    assert!(reason.contains("Required status check"), "{reason}");
    assert_eq!(pull(&github).state, GithubPullState::Open);

    github.refuse_merges(None);
    let retried = send(&env, "POST", "/api/items/app%231/merge", &json!({}));
    assert_eq!(retried.status, 200, "{}", retried.body);
    assert_eq!(json_of(&retried)["outcome"]["kind"], "merged");
    assert!(matches!(
        pull(&github).state,
        GithubPullState::Merged { .. }
    ));
}

#[test]
fn f8_the_inbox_answers_while_github_merges() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github);
    github.hold_merges(true);

    std::thread::scope(|scope| {
        let approved = scope.spawn(|| approve(&env, 1));
        let merging = |r: &String| r.starts_with("PUT ") && r.ends_with("/merge");
        while !github.requests().iter().any(merging) {
            assert!(!approved.is_finished(), "the merge never reached GitHub");
            std::thread::sleep(Duration::from_millis(10));
        }
        let (sent, inbox) = mpsc::channel();
        let addr = env.hub_addr();
        scope.spawn(move || {
            let request =
                format!("GET /api/inbox HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
            let _ = sent.send(json_of(&http_reply(&addr, &request)));
        });
        let inbox = inbox.recv_timeout(Duration::from_secs(10));
        // Let through before failing, so the scope's threads can end.
        github.hold_merges(false);
        let inbox = inbox.expect("the inbox waited for GitHub to merge");
        // The approval is recorded before the item lands.
        assert_eq!(inbox["merging"][0]["id"], "app#1", "{inbox}");
        assert_eq!(approved.join().unwrap()["outcome"]["kind"], "merged");
    });
}

#[test]
fn f8_a_pull_request_merged_on_github_merges_the_item() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github);
    let commit = github.merge_on_github(ACME, 1);

    // The poller finds it in time; a sync finds it at once.
    env.mg(&["--json", "sync", "app#1"]).assert_code(0);
    assert_eq!(item(&env)["state"], "merged");
    let wait = env.mg(&["wait", "app#1"]).assert_code(0).stdout;
    assert!(
        wait.contains(&format!(
            "item.merged app#1 r1: it was merged on GitHub as {commit}"
        )),
        "{wait}"
    );
    let replay = env
        .mg(&["--json", "feedback", "app#1", "--since", "0"])
        .assert_code(0)
        .json();
    assert_eq!(replay["events"][0]["source"], "external", "{replay}");
    // As after a merge through the hub, the branch goes from GitHub.
    assert_eq!(github.rev(ACME, "login"), None);

    // Approving now finds nothing left to merge.
    let reply = send(
        &env,
        "POST",
        "/api/items/app%231/approve",
        &json!({ "revision": 1, "summary": "" }),
    );
    assert_eq!(reply.status, 409, "{}", reply.body);
    assert_eq!(json_of(&reply)["error"]["code"], "item_closed");
}

#[test]
fn f14_a_pull_request_closed_on_github_abandons_the_item() {
    let github = FakeGithub::start();
    let (env, repo) = submitted(&github);
    github.close_on_github(ACME, 1);

    env.mg(&["--json", "sync", "app#1"]).assert_code(0);
    assert_eq!(item(&env)["state"], "abandoned");
    let wait = env.mg(&["wait", "app#1"]).assert_code(0).stdout;
    assert!(
        wait.contains(
            "item.abandoned app#1 r1: GitHub abandoned it\n  \
             Its pull request was closed on GitHub without merging."
        ),
        "{wait}"
    );
    // Whoever closed it may reopen it on GitHub, so its branch stays there.
    assert_eq!(github.rev(ACME, "login"), Some(repo.rev("login")));
}

#[test]
fn f9_a_conflict_on_github_sends_the_item_back_to_rebase() {
    let github = FakeGithub::start();
    let (env, repo) = submitted(&github);
    repo.checkout("main");
    repo.commit_files("Login too", &[("src/login.rs", "fn login(x: u8) {}\n")]);
    push_to_github(&github, &repo, "main");
    let target = repo.rev("main");
    repo.checkout("login");

    let merge = approve(&env, 1);
    assert_eq!(
        merge["outcome"],
        json!({ "kind": "conflict", "target_commit": target, "paths": ["src/login.rs"] })
    );
    assert_eq!(merge["item"]["state"], "changes_requested");
    assert_eq!(pull(&github).state, GithubPullState::Open);

    // The agent's rebase goes out as the next revision of the same PR.
    repo.git(["fetch", "--quiet", "mergegate"]);
    repo.git(["rebase", "--quiet", "-X", "theirs", "mergegate/main"]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    assert_eq!(approve(&env, 2)["outcome"]["kind"], "merged");
    assert_eq!(github.rev(ACME, "main^").as_deref(), Some(target.as_str()));
}

#[test]
fn f14_abandoning_closes_the_pull_request() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github);
    let reply = send(
        &env,
        "POST",
        "/api/items/app%231/abandon",
        &json!({ "source": "operator", "reason": "Not needed." }),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(json_of(&reply)["state"], "abandoned");
    assert_eq!(pull(&github).state, GithubPullState::Closed);
    assert_eq!(github.rev(ACME, "login"), None);
}

#[test]
fn f14_the_operator_abandons_an_item_whose_repository_github_deleted() {
    let github = FakeGithub::start();
    let (env, _repo) = submitted(&github);
    github.delete_repo(ACME);
    let abandon = |body: Value| send(&env, "POST", "/api/items/app%231/abandon", &body);

    let reply = abandon(json!({ "source": "operator" }));
    assert_eq!(reply.status, 502, "{}", reply.body);
    assert_eq!(error_code(&json_of(&reply)), "github_no_access");
    // Only the operator may leave a pull request open on GitHub.
    let reply = abandon(json!({ "source": "agent", "leave_github": true }));
    assert_eq!(reply.status, 403, "{}", reply.body);
    assert_eq!(error_code(&json_of(&reply)), "operator_only");
    assert_eq!(item(&env)["state"], "needs_review");

    let reply = abandon(json!({ "source": "operator", "leave_github": true }));
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(json_of(&reply)["state"], "abandoned");
}

#[test]
fn f14_abandoning_waits_for_github() {
    let github = FakeGithub::start();
    let (env, repo) = submitted(&github);
    github.log_out();
    let failed = env
        .mg_at(repo.root(), &["--json", "abandon", "app#1"])
        .assert_code(1)
        .json();
    assert_eq!(error_code(&failed), "github_login");
    assert_eq!(pull(&github).state, GithubPullState::Open);
    github.log_in("gho_second");
    env.mg_at(repo.root(), &["--json", "abandon", "app#1"])
        .assert_code(0);
    assert_eq!(pull(&github).state, GithubPullState::Closed);
}

#[test]
fn f15_switching_to_github_keeps_open_items_local() {
    let github = FakeGithub::start();
    let env = env(&github);
    env.start_hub();
    let repo = on_github(&github);
    let root: &Path = repo.root();
    env.mg_at(root, &["--json", "project", "add", "--backend", "local"])
        .assert_code(0);
    env.mg_at(root, &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "login"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    env.mg_at(root, &["--json", "submit"]).assert_code(0);

    let asked = env
        .mg(&["--json", "project", "set", "app", "backend", "github"])
        .assert_code(2)
        .json();
    assert_eq!(asked["needs_decision"]["flag"], "--confirm");
    assert_eq!(
        asked["needs_decision"]["question"],
        "1 open item keeps the local backend until it closes: app#1 (login). \
         New items will land through pull requests on acme/app. Switch?"
    );
    let switched = env
        .mg(&[
            "--json",
            "project",
            "set",
            "app",
            "backend",
            "github",
            "--confirm",
        ])
        .assert_code(0)
        .json();
    assert_eq!(
        (&switched["backend"], &switched["github"]),
        (&json!("github"), &json!(ACME))
    );

    // The open item still lands locally; a new one opens a pull request.
    let base = repo.rev("main");
    assert_eq!(approve(&env, 1)["outcome"]["kind"], "merged");
    assert_eq!(repo.rev("main^"), base);
    assert!(github.pulls().is_empty());
    repo.git(["switch", "--quiet", "-c", "search", "main"]);
    push_to_github(&github, &repo, "main");
    repo.commit_files("Add search", &[("src/search.rs", "\n")]);
    let submitted = env.mg_at(root, &["--json", "submit"]).assert_code(0).json();
    assert_eq!(submitted["item"]["id"], "app#2");
    assert_eq!(submitted["item"]["link"]["label"], "#1");
}
