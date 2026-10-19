//! Submitting work: workers push to the hub and register items and
//! revisions, and the inbox lists them. Also a project's settings changed under open items, against a real
//! hub and real repos.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use testkit::{Env, TestRepo, commit_files, git, http};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn error_code(run: &Value) -> &str {
    run["error"]["code"].as_str().unwrap_or_default()
}

fn staging(env: &Env, project: &str) -> PathBuf {
    env.home()
        .join("projects")
        .join(project)
        .join("staging.git")
}

fn rev(dir: &Path, rev: &str) -> String {
    git(dir, ["rev-parse", rev]).trim().to_owned()
}

/// A hub with `app` registered and its repo's `mergegate` remote added.
fn hub_with_app() -> (Env, TestRepo) {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    (env, repo)
}

fn submit(env: &Env, dir: &Path) -> Value {
    env.mg_at(dir, &["--json", "submit"]).json()
}

#[test]
fn f4_submits_a_branch_and_its_revisions() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);

    let init = env
        .mg_at(repo.root(), &["--json", "init"])
        .assert_code(0)
        .json();
    let url = format!("http://{}/git/app", env.hub_addr());
    assert_eq!(
        init,
        json!({"project": "app", "remote": "mergegate", "url": url})
    );
    assert_eq!(repo.git(["remote", "get-url", "mergegate"]).trim(), url);

    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files(
        "Add the parser",
        &[
            ("src/parser.rs", "fn parse() {}\n"),
            ("README", "app\nparser\n"),
        ],
    );
    let head = repo.rev("HEAD");
    let submitted = env
        .mg_at(repo.root(), &["--json", "submit"])
        .assert_code(0)
        .json();
    let item = &submitted["item"];
    assert_eq!(submitted["new_revision"], true);
    assert_eq!(item["id"], "app#1");
    assert_eq!(item["title"], "Add the parser");
    assert_eq!(item["branch"], "feature");
    assert_eq!(item["state"], "needs_review");
    assert_eq!(item["backend"], "local");
    assert_eq!(item["target"], "main");
    let r1 = &item["revision"];
    assert_eq!(r1["number"], 1);
    assert_eq!(r1["head"], head);
    assert_eq!(r1["base"], repo.rev("main"));
    assert_eq!(
        (&r1["files"], &r1["additions"], &r1["deletions"]),
        (&json!(2), &json!(3), &json!(0))
    );
    let staging = staging(&env, "app");
    assert_eq!(rev(&staging, "feature"), head);
    assert_eq!(rev(&staging, "refs/mergegate/items/1/r1"), head);

    // The same head again is the same revision.
    let again = env.mg_at(repo.root(), &["submit"]).assert_code(0).stdout;
    assert_eq!(
        again.trim_end(),
        format!("app#1 r1 is already at {}: Add the parser", &head[..12])
    );

    repo.commit_files(
        "Parse numbers",
        &[("src/parser.rs", "fn parse() -> u32 { 0 }\n")],
    );
    let text = env.mg_at(repo.root(), &["submit"]).assert_code(0).stdout;
    assert_eq!(
        text.trim_end(),
        "submitted app#1 r2: Add the parser\n  feature onto main · 2 files, +3 -0"
    );
    assert_eq!(rev(&staging, "refs/mergegate/items/1/r2"), repo.rev("HEAD"));
    assert_eq!(rev(&staging, "refs/mergegate/items/1/r1"), head);

    // A rewritten branch is a new revision too, and r2 stays pinned. A title
    // given again renames the item.
    let r2 = repo.rev("HEAD");
    repo.git([
        "commit",
        "--quiet",
        "--amend",
        "-m",
        "Parse numbers, really",
    ]);
    let r3 = env
        .mg_at(
            repo.root(),
            &["--json", "submit", "--title", "Number parser"],
        )
        .assert_code(0)
        .json();
    assert_eq!(r3["item"]["revision"]["number"], 3);
    assert_eq!(r3["item"]["title"], "Number parser");
    assert_eq!(rev(&staging, "refs/mergegate/items/1/r2"), r2);
}

#[test]
fn f4_lists_open_items_in_the_inbox() {
    let (env, repo) = hub_with_app();
    let mut empty = env.mg(&["--json", "inbox"]).assert_code(0).json();
    let weekly = empty["weekly"].take();
    assert_eq!(weekly["repositories"], json!([]));
    assert_eq!(weekly["monday"].as_str().unwrap().len(), 10);
    assert_eq!(weekly["today"].as_str().unwrap().len(), 10);
    assert_eq!(
        empty,
        json!({"needs_you": [], "with_agents": [], "merging": [], "closed": [], "weekly": null, "rules": []})
    );
    assert_eq!(
        env.mg(&["inbox"]).assert_code(0).stdout.trim_end(),
        "no open items"
    );

    for branch in ["one", "two"] {
        repo.checkout("main");
        repo.git(["switch", "--quiet", "-c", branch]);
        repo.commit_files(&format!("Work on {branch}"), &[(branch, "a\nb\n")]);
        env.mg_at(
            repo.root(),
            &[
                "--json",
                "submit",
                "--title",
                &format!("The {branch} change"),
            ],
        )
        .assert_code(0);
    }

    let inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    let ids: Vec<&Value> = inbox["needs_you"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| &i["id"])
        .collect();
    assert_eq!(ids, [&json!("app#1"), &json!("app#2")]);
    let first = &inbox["needs_you"][0];
    assert_eq!(first["title"], "The one change");
    assert_eq!(first["branch"], "one");
    assert_eq!(
        (
            &first["revision"],
            &first["files"],
            &first["additions"],
            &first["deletions"]
        ),
        (&json!(1), &json!(1), &json!(2), &json!(0))
    );
    assert_eq!(inbox["with_agents"], json!([]));
    assert_eq!(inbox["merging"], json!([]));

    let text = env.mg(&["inbox"]).assert_code(0).stdout;
    assert_eq!(
        text.trim_end(),
        "needs the operator (2)\n  \
         app#1 r1  The one change  (one · 1 file, +2 -0)\n  \
         app#2 r1  The two change  (two · 1 file, +2 -0)"
    );
}

#[test]
fn f4_works_from_a_clone_that_fetches_from_the_hub() {
    let (env, repo) = hub_with_app();
    let clone = repo.sibling("clone");
    git(
        repo.root().parent().unwrap(),
        ["clone", "--quiet", repo.root().to_str().unwrap(), "clone"],
    );
    let init = env
        .mg_at(&clone, &["--json", "init", "--project", "app"])
        .assert_code(0)
        .json();
    assert_eq!(init["project"], "app");

    git(&clone, ["switch", "--quiet", "-c", "feature"]);
    commit_files(&clone, "Clone work", &[("clone.txt", "hi\n")]);
    let submitted = submit(&env, &clone);
    assert_eq!(submitted["item"]["id"], "app#1", "{submitted}");

    // Workers fetch from the hub, but not its pins.
    let advertised = git(&clone, ["ls-remote", "mergegate"]);
    assert!(advertised.contains("refs/heads/feature"), "{advertised}");
    assert!(!advertised.contains("refs/mergegate"), "{advertised}");

    // The target follows the upstream, and a push to it doesn't stick.
    repo.commit_files("Upstream moves", &[("upstream.txt", "new\n")]);
    git(&clone, ["push", "--quiet", "mergegate", "+feature:main"]);
    git(&clone, ["fetch", "--quiet", "mergegate"]);
    assert_eq!(rev(&clone, "mergegate/main"), repo.rev("main"));
    assert_eq!(rev(&clone, "mergegate/feature"), rev(&clone, "feature"));
}

#[test]
fn f4_pushes_large_files() {
    let (env, repo) = hub_with_app();
    repo.git(["switch", "--quiet", "-c", "assets"]);
    // Incompressible, so the pack stays bigger than any default body limit.
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let blob: Vec<u8> = (0..3 << 20)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect();
    std::fs::write(repo.root().join("blob.bin"), blob).unwrap();
    repo.git(["add", "blob.bin"]);
    repo.git(["commit", "--quiet", "-m", "Add a blob"]);

    let submitted = submit(&env, repo.root());
    assert_eq!(submitted["item"]["revision"]["files"], 1, "{submitted}");
    assert_eq!(rev(&staging(&env, "app"), "assets"), repo.rev("HEAD"));
}

#[test]
fn f4_refuses_what_it_cannot_submit() {
    let (env, repo) = hub_with_app();

    let target = submit(&env, repo.root());
    assert_eq!(error_code(&target), "target_branch", "{target}");
    assert_eq!(target["error"]["hint"], "git switch -c NAME");

    repo.git(["switch", "--quiet", "-c", "empty"]);
    let nothing = submit(&env, repo.root());
    assert_eq!(error_code(&nothing), "nothing_to_submit", "{nothing}");

    repo.git(["checkout", "--quiet", "--detach"]);
    let detached = submit(&env, repo.root());
    assert_eq!(error_code(&detached), "detached_head", "{detached}");

    let other = TestRepo::new("other");
    let no_remote = submit(&env, other.root());
    assert_eq!(error_code(&no_remote), "no_remote", "{no_remote}");
    assert_eq!(no_remote["error"]["hint"], "mg init");
    let unregistered = env.mg_at(other.root(), &["--json", "init"]).json();
    assert_eq!(error_code(&unregistered), "no_project", "{unregistered}");

    let elsewhere = submit(&env, env.home());
    assert_eq!(error_code(&elsewhere), "not_a_repo", "{elsewhere}");

    // The hub checks the branch is pushed at the head it's told about.
    let addr = env.hub_addr();
    let body = json!({"project": "app", "branch": "ghost", "head": repo.rev("HEAD")}).to_string();
    let (status, reply) = http(
        &addr,
        &format!(
            "POST /api/items HTTP/1.1\r\nHost: {addr}\r\nx-mergegate-client: test\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    );
    assert_eq!(status, 409, "{reply}");
    assert!(reply.contains("not_pushed"), "{reply}");
}

#[test]
fn git_requests_pass_the_guard_only_as_git() {
    let (env, _repo) = hub_with_app();
    let addr = env.hub_addr();
    let post = |content_type: &str| {
        http(
            &addr,
            &format!(
                "POST /git/app/git-receive-pack HTTP/1.1\r\nHost: {addr}\r\n\
                 Content-Type: {content_type}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            ),
        )
    };
    let (status, body) = post("text/plain");
    assert_eq!(status, 403, "{body}");
    let (status, body) = post("application/x-git-receive-pack-request");
    assert_ne!(status, 403, "{body}");

    let (status, body) = http(
        &addr,
        &format!(
            "GET /git/nope/info/refs?service=git-upload-pack HTTP/1.1\r\nHost: {addr}\r\n\
             Connection: close\r\n\r\n"
        ),
    );
    assert_eq!(status, 404, "{body}");
}

#[test]
fn f15_asks_before_moving_the_target_away_from_open_items() {
    let (env, repo) = hub_with_app();
    repo.branch("develop");
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Feature", &[("f", "1\n")]);
    submit(&env, repo.root());

    let asked = env
        .mg(&["--json", "project", "set", "app", "target", "develop"])
        .assert_code(2)
        .json();
    assert_eq!(
        asked,
        json!({"needs_decision": {
            "question": "1 open item keeps its target until it closes: \
                         app#1 (feature onto main). New items will target develop. Change it?",
            "flag": "--confirm",
            "options": []
        }})
    );
    let unchanged = env.mg(&["--json", "project", "show", "app"]).json();
    assert_eq!(unchanged["target"], "main");

    let set = env
        .mg(&[
            "--json",
            "project",
            "set",
            "app",
            "target",
            "develop",
            "--confirm",
        ])
        .assert_code(0)
        .json();
    assert_eq!(set["target"], "develop");

    // The open item keeps its target; new items get the new one.
    repo.commit_files("More", &[("f", "2\n")]);
    let r2 = submit(&env, repo.root());
    assert_eq!(r2["item"]["target"], "main", "{r2}");
    repo.checkout("develop");
    repo.git(["switch", "--quiet", "-c", "next"]);
    repo.commit_files("Next", &[("n", "1\n")]);
    let next = submit(&env, repo.root());
    assert_eq!(next["item"]["target"], "develop", "{next}");

    // Back to main needs asking only about the item on develop.
    let back = env
        .mg(&["--json", "project", "set", "app", "target", "main"])
        .assert_code(2)
        .json();
    let question = back["needs_decision"]["question"].as_str().unwrap();
    assert!(question.contains("app#2 (next onto develop)"), "{question}");
    assert!(!question.contains("app#1"), "{question}");

    // An open item's branch can't become the target, even confirmed.
    let taken = env
        .mg(&[
            "--json",
            "project",
            "set",
            "app",
            "target",
            "next",
            "--confirm",
        ])
        .assert_code(1)
        .json();
    assert_eq!(error_code(&taken), "branch_under_review", "{taken}");
}

#[test]
fn f4_submits_a_branch_a_tag_shares_its_name_with() {
    let (env, repo) = hub_with_app();
    repo.git(["tag", "main"]);
    let target = submit(&env, repo.root());
    assert_eq!(error_code(&target), "target_branch", "{target}");

    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Feature", &[("f", "1\n")]);
    repo.git(["tag", "feature"]);
    let submitted = submit(&env, repo.root());
    assert_eq!(submitted["item"]["branch"], "feature", "{submitted}");
    assert_eq!(
        rev(&staging(&env, "app"), "refs/heads/feature"),
        repo.rev("HEAD")
    );
}

#[test]
fn f4_retitles_an_item_without_new_commits() {
    let (env, repo) = hub_with_app();
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Feature", &[("f", "1\n")]);
    submit(&env, repo.root());
    let again = env
        .mg_at(
            repo.root(),
            &["--json", "submit", "--title", " Login page "],
        )
        .assert_code(0)
        .json();
    assert_eq!(again["new_revision"], false, "{again}");
    assert_eq!(again["item"]["title"], "Login page");
    let inbox = env.mg(&["--json", "inbox"]).assert_code(0).json();
    assert_eq!(inbox["needs_you"][0]["title"], "Login page", "{inbox}");
}

#[test]
fn f4_names_the_open_branch_a_new_one_nests_with() {
    let (env, repo) = hub_with_app();
    repo.git(["switch", "--quiet", "-c", "auth"]);
    repo.commit_files("Auth", &[("auth.rs", "1\n")]);
    assert_eq!(submit(&env, repo.root())["item"]["id"], "app#1");

    // Git can't hold `auth/login` beside `auth`, so neither can the hub
    // while app#1 is open.
    repo.git(["switch", "--quiet", "--detach", "main"]);
    repo.git(["branch", "-D", "auth"]);
    repo.git(["switch", "--quiet", "-c", "auth/login"]);
    repo.commit_files("Login", &[("login.rs", "1\n")]);
    let under = submit(&env, repo.root());
    assert_eq!(error_code(&under), "branch_nested", "{under}");
    assert_eq!(
        under["error"]["message"],
        "the hub has a branch auth, so it can't take auth/login: one branch name can't nest in another"
    );
    assert_eq!(under["error"]["hint"], "git branch -m NAME");
}

#[test]
fn f4_names_the_open_branch_a_new_one_nests_with_once_refs_are_packed() {
    let (env, repo) = hub_with_app();
    repo.git(["switch", "--quiet", "-c", "auth"]);
    repo.commit_files("Auth", &[("auth.rs", "1\n")]);
    assert_eq!(submit(&env, repo.root())["item"]["id"], "app#1");
    // Receive-pack's `gc --auto` packs staging's refs, and git words the
    // refusal differently for a packed one.
    git(&staging(&env, "app"), ["pack-refs", "--all"]);

    repo.git(["switch", "--quiet", "--detach", "main"]);
    repo.git(["branch", "-D", "auth"]);
    repo.git(["switch", "--quiet", "-c", "auth/login"]);
    repo.commit_files("Login", &[("login.rs", "1\n")]);
    let under = submit(&env, repo.root());
    assert_eq!(error_code(&under), "branch_nested", "{under}");
    assert_eq!(
        under["error"]["message"],
        "the hub has a branch auth, so it can't take auth/login: one branch name can't nest in another"
    );
}
