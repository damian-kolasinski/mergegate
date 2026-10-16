//! Stacked items (F16): an item submitted `--on` another is reviewed as its
//! own commits, lands right after its parent, and is told how to move its
//! commits whenever its parent moves on, on either backend.

use std::process::Command;

use serde_json::{Value, json};
use testkit::{Env, FakeGithub, GithubPullState, Reply, TestRepo, http_reply};

/// Without the target watch: these tests are of what merging finds, which
/// the watch could find first.
fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(vec![("MG_TARGET_WATCH".into(), "off".into())])
}

fn send(env: &Env, method: &str, path: &str, body: Option<&Value>) -> Reply {
    let addr = env.hub_addr();
    let body = body.map(Value::to_string).unwrap_or_default();
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

fn path(id: &str) -> String {
    id.replace('#', "%23")
}

fn approve(env: &Env, id: &str, revision: u32) -> Value {
    let body = json!({ "revision": revision, "summary": "" });
    let reply = send(
        env,
        "POST",
        &format!("/api/items/{}/approve", path(id)),
        Some(&body),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    json_of(&reply)
}

/// `GET /api/items/{id}`: the item with its own diff and its stack.
fn shown(env: &Env, id: &str) -> Value {
    let reply = send(env, "GET", &format!("/api/items/{}", path(id)), None);
    assert_eq!(reply.status, 200, "{}", reply.body);
    json_of(&reply)
}

fn paths(shown: &Value) -> Vec<&str> {
    let files = shown["files"].as_array().unwrap();
    files.iter().map(|f| f["path"].as_str().unwrap()).collect()
}

/// Every event `id`'s agent has heard, whatever it's read already.
fn events(env: &Env, id: &str) -> Vec<Value> {
    let feedback = env.mg(&["--json", "feedback", id, "--since", "0"]).json();
    feedback["events"].as_array().cloned().unwrap_or_default()
}

/// Runs an event's commands in the agent's checkout, as its agent would.
fn run(repo: &TestRepo, commands: &Value) {
    for command in commands.as_array().unwrap() {
        let args: Vec<_> = command.as_str().unwrap().split(' ').collect();
        assert_eq!(args[0], "git");
        repo.git(&args[1..]);
    }
}

/// `app`, registered and initialized.
fn project(env: &Env, repo: &TestRepo, args: &[&str]) {
    env.start_hub();
    let mut add = vec!["--json", "project", "add"];
    add.extend_from_slice(args);
    env.mg_at(repo.root(), &add).assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
}

/// `app#1` adding `src/login.rs` on `login`, and `app#2` adding
/// `src/profile.rs` on `profile`, built on `login` and stacked on it. The
/// checkout is left on `profile`.
fn stacked(env: &Env, repo: &TestRepo) -> Value {
    repo.git(["switch", "--quiet", "-c", "login"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "profile"]);
    repo.commit_files("Add profile", &[("src/profile.rs", "fn profile() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit", "--on", "1"])
        .assert_code(0)
        .json()
}

fn local() -> (Env, TestRepo) {
    let env = env();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    project(&env, &repo, &[]);
    (env, repo)
}

#[test]
fn f16_a_stacked_item_is_its_own_diff_and_lands_right_after_its_parent() {
    let (env, repo) = local();
    let submitted = stacked(&env, &repo);
    let login = repo.rev("login");
    assert_eq!(submitted["item"]["id"], "app#2");
    assert_eq!(submitted["item"]["parent"], "app#1");
    assert_eq!(submitted["item"]["revision"]["base"], login.as_str());

    // Reviewed as its own commits, in a stack read bottom first.
    let child = shown(&env, "app#2");
    assert_eq!(paths(&child), ["src/profile.rs"]);
    assert_eq!(
        child["stack"],
        json!([
            { "id": "app#1", "title": "Add login", "state": "needs_review" },
            { "id": "app#2", "title": "Add profile", "state": "needs_review" },
        ])
    );
    assert_eq!(child["behind"], false);
    let inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    let needs_you: Vec<_> = inbox["needs_you"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["id"].clone(), i["parent"].clone()))
        .collect();
    assert_eq!(
        needs_you,
        [
            (json!("app#1"), Value::Null),
            (json!("app#2"), json!("app#1"))
        ]
    );

    // Approved first, it waits for its parent, untried.
    let main = repo.rev("main");
    let waiting = approve(&env, "app#2", 1);
    assert_eq!(
        waiting["outcome"],
        json!({ "kind": "waiting", "parent": "app#1" })
    );
    assert_eq!(waiting["item"]["state"], "merging");
    assert_eq!(repo.rev("main"), main);
    let inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    assert_eq!(
        inbox["merging"][0]["blocked"],
        json!({ "kind": "waiting", "parent": "app#1" })
    );

    // The parent's merge carries it: two commits, bottom first.
    let merged = approve(&env, "app#1", 1);
    assert_eq!(merged["item"]["state"], "merged");
    assert_eq!(merged["stacked"][0]["id"], "app#2");
    assert_eq!(merged["stacked"][0]["state"], "merged");
    let log = repo.git(["log", "--format=%s", &format!("{main}..main")]);
    assert_eq!(log.trim(), "Add profile (app#2)\nAdd login (app#1)");
    assert_eq!(repo.rev("main^{tree}"), repo.rev("profile^{tree}"));
    let kinds: Vec<_> = events(&env, "app#2")
        .iter()
        .map(|e| e["type"].clone())
        .collect();
    assert_eq!(kinds, [json!("review.submitted"), json!("item.merged")]);
    // A merged item still shows its own diff.
    assert_eq!(paths(&shown(&env, "app#2")), ["src/profile.rs"]);
}

#[test]
fn f16_stacks_are_linear_and_built_on_their_parent() {
    let (env, repo) = local();
    stacked(&env, &repo);

    // A second child of app#1 would leave the first's base ambiguous.
    repo.git(["switch", "--quiet", "-c", "avatar", "login"]);
    repo.commit_files("Add avatar", &[("src/avatar.rs", "fn avatar() {}\n")]);
    let refused = env
        .mg_at(repo.root(), &["--json", "submit", "--on", "app#1"])
        .assert_code(1)
        .json();
    assert_eq!(refused["error"]["code"], "has_child", "{refused}");
    assert_eq!(refused["error"]["hint"], "mg submit --on app#2");

    // Built off main, it has none of app#1's commits to be stacked on.
    repo.git(["switch", "--quiet", "-c", "settings", "main"]);
    repo.commit_files("Add settings", &[("src/settings.rs", "fn settings() {}\n")]);
    let refused = env
        .mg_at(repo.root(), &["--json", "submit", "--on", "2"])
        .assert_code(1)
        .json();
    assert_eq!(refused["error"]["code"], "not_on_parent", "{refused}");
    assert_eq!(refused["error"]["hint"], "git rebase mergegate/profile");

    let refused = env
        .mg_at(repo.root(), &["--json", "submit", "--on", "app#x"])
        .assert_code(1)
        .json();
    assert_eq!(refused["error"]["code"], "invalid_item", "{refused}");
    assert_eq!(
        env.mg(&["--json", "inbox"]).json()["needs_you"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn f16_a_new_parent_revision_tells_the_child_how_to_rebase_onto_it() {
    let (env, repo) = local();
    stacked(&env, &repo);
    let old = repo.rev("login");
    repo.checkout("login");
    repo.commit_files(
        "Take a user",
        &[("src/login.rs", "fn login(user: &str) {}\n")],
    );
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    let new = repo.rev("login");

    let child = shown(&env, "app#2");
    assert_eq!(child["behind"], true);
    let inbox = env.mg(&["--json", "inbox"]).json();
    assert_eq!(inbox["needs_you"][1]["behind"], true, "{inbox}");
    let events = events(&env, "app#2");
    assert_eq!(
        events[0],
        json!({
            "cursor": events[0]["cursor"],
            "type": "parent.updated",
            "source": "agent",
            "item": "app#2",
            "revision": 1,
            "parent": "app#1",
            "merged": false,
            "onto": new,
            "upstream": old,
            "commands": [
                "git fetch mergegate",
                format!("git rebase --onto {new} {old}"),
            ],
        })
    );

    // The commands move only the child's own commits.
    repo.checkout("profile");
    run(&repo, &events[0]["commands"]);
    let resubmitted = env
        .mg_at(repo.root(), &["--json", "submit"])
        .assert_code(0)
        .json();
    assert_eq!(resubmitted["item"]["revision"]["number"], 2);
    assert_eq!(resubmitted["item"]["revision"]["base"], new.as_str());
    assert_eq!(resubmitted["item"]["parent"], "app#1");
    let child = shown(&env, "app#2");
    assert_eq!(child["behind"], false);
    assert_eq!(paths(&child), ["src/profile.rs"]);
}

#[test]
fn f16_a_child_that_doesnt_apply_after_its_parent_waits_for_its_agent() {
    let (env, repo) = local();
    repo.git(["switch", "--quiet", "-c", "login"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "profile"]);
    repo.commit_files("Retitle", &[("README", "profile\n")]);
    env.mg_at(repo.root(), &["--json", "submit", "--on", "1"])
        .assert_code(0);
    let upstream = repo.rev("login");
    // The target moves under the child's own change, not its parent's.
    repo.checkout("main");
    repo.commit_files("Retitle too", &[("README", "other\n")]);
    repo.checkout("profile");

    approve(&env, "app#2", 1);
    let merged = approve(&env, "app#1", 1);
    assert_eq!(merged["item"]["state"], "merged");
    assert_eq!(merged["stacked"][0]["state"], "merging");
    let target = repo.rev("main");
    assert_eq!(
        shown(&env, "app#2")["blocked"],
        json!({ "kind": "behind", "parent": "app#1" })
    );
    let text = env.mg(&["feedback", "app#2"]).assert_code(0).stdout;
    assert!(
        text.contains("app#1, which this is stacked on, merged"),
        "{text}"
    );
    let notice = events(&env, "app#2").pop().unwrap();
    assert_eq!(notice["type"], "parent.updated");
    assert_eq!(notice["merged"], true);
    assert_eq!(notice["onto"], target.as_str());
    assert_eq!(
        notice["commands"],
        json!([
            "git fetch mergegate",
            format!("git rebase --onto mergegate/main {upstream}"),
        ])
    );

    // The agent resolves the conflict the rebase stops at.
    repo.git(["fetch", "mergegate"]);
    let rebase = Command::new("git")
        .current_dir(repo.root())
        .args(["rebase", "--onto", "mergegate/main", &upstream])
        .output()
        .unwrap();
    assert!(!rebase.status.success(), "the rebase applied cleanly");
    std::fs::write(repo.root().join("README"), "profile\n").unwrap();
    repo.git(["add", "README"]);
    repo.git(["-c", "core.editor=true", "rebase", "--continue"]);
    let resubmitted = env
        .mg_at(repo.root(), &["--json", "submit"])
        .assert_code(0)
        .json();
    // Its parent merged, so the new revision is on the target.
    assert_eq!(resubmitted["item"]["parent"], Value::Null);
    assert_eq!(resubmitted["item"]["revision"]["base"], target.as_str());
    let child = shown(&env, "app#2");
    assert_eq!(child["blocked"], Value::Null);
    assert_eq!(paths(&child), ["README"]);
}

#[test]
fn f16_an_abandoned_parent_hands_its_child_back_onto_the_target() {
    let (env, repo) = local();
    stacked(&env, &repo);
    let upstream = repo.rev("login");
    approve(&env, "app#2", 1);
    env.mg(&["abandon", "app#1"]).assert_code(0);

    let notice = events(&env, "app#2").pop().unwrap();
    assert_eq!(notice["type"], "parent.abandoned");
    assert_eq!(notice["parent"], "app#1");
    assert_eq!(
        notice["commands"],
        json!([
            "git fetch mergegate",
            format!("git rebase --onto mergegate/main {upstream}"),
        ])
    );
    let child = shown(&env, "app#2");
    assert_eq!(child["behind"], true);
    assert_eq!(child["item"]["state"], "changes_requested");

    run(&repo, &notice["commands"]);
    assert!(!repo.root().join("src/login.rs").exists());
    let resubmitted = env
        .mg_at(repo.root(), &["--json", "submit"])
        .assert_code(0)
        .json();
    assert_eq!(resubmitted["item"]["parent"], Value::Null);
    assert_eq!(
        resubmitted["item"]["revision"]["base"],
        repo.rev("main").as_str()
    );
    // The same own diff, but on the target now, so the operator looks again.
    assert_eq!(resubmitted["item"]["state"], "needs_review");
    let child = shown(&env, "app#2");
    assert_eq!(child["stack"], json!([]));
    assert_eq!(paths(&child), ["src/profile.rs"]);
}

#[test]
fn f16_on_github_a_child_pull_request_is_based_on_its_parents_branch_until_it_lands() {
    let github = FakeGithub::start();
    let env = Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(github.env());
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    repo.git(["remote", "add", "origin", "git@github.com:acme/app.git"]);
    github.create_repo("acme/app", repo.root());
    project(&env, &repo, &["--backend", "github"]);
    stacked(&env, &repo);
    let pulls = github.pulls();
    assert_eq!(
        (pulls[0].branch.as_str(), pulls[0].base.as_str()),
        ("login", "main")
    );
    assert_eq!(
        (pulls[1].branch.as_str(), pulls[1].base.as_str()),
        ("profile", "login")
    );

    approve(&env, "app#2", 1);
    let merged = approve(&env, "app#1", 1);
    assert_eq!(merged["stacked"][0]["state"], "merged", "{merged}");
    // Retargeted before its parent's branch went, which would close it.
    let pulls = github.pulls();
    assert_eq!(pulls[1].base, "main");
    for pull in &pulls {
        assert!(
            matches!(pull.state, GithubPullState::Merged { .. }),
            "{pull:?}"
        );
    }
    assert_eq!(github.rev("acme/app", "login"), None);
    let tree = github.git("acme/app", ["rev-parse", "main^{tree}"]);
    assert_eq!(tree.trim(), repo.rev("profile^{tree}"));
}
