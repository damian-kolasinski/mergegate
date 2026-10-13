//! Projects: registering a repo with the local backend (F1) and changing its
//! settings later (F15), against a real hub and real repos.

use std::path::Path;

use serde_json::{Value, json};
use testkit::{Env, TestRepo, git};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn error_code(run: &Value) -> &str {
    run["error"]["code"].as_str().unwrap_or_default()
}

/// The commit `branch` points at in the project's staging repo.
fn staged(env: &Env, project: &str, branch: &str) -> String {
    let staging = env
        .home()
        .join("projects")
        .join(project)
        .join("staging.git");
    git(&staging, ["rev-parse", branch]).trim().to_owned()
}

fn check<'a>(doctor: &'a Value, name: &str) -> &'a Value {
    doctor["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no {name} check in {doctor}"))
}

#[test]
fn f1_local_registers_the_repo_and_mirrors_its_target() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");

    // Nothing is registered here yet, so show lists every project: none.
    let shown = env.mg_at(repo.root(), &["--json", "project", "show"]);
    assert_eq!(shown.assert_code(0).json(), json!({"projects": []}));

    let added = env
        .mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0)
        .json();
    assert_eq!(
        added,
        json!({
            "name": "app",
            "backend": "local",
            "target": "main",
            "upstream": path(repo.root()),
        })
    );
    assert_eq!(staged(&env, "app", "main"), repo.rev("main"));

    // Agents find the project from anywhere in the repo, worktrees included.
    let subdir = repo.root().join("src");
    std::fs::create_dir(&subdir).unwrap();
    for dir in [repo.root().to_owned(), subdir, repo.worktree("feature")] {
        let shown = env.mg_at(&dir, &["--json", "project", "show"]);
        assert_eq!(shown.assert_code(0).json(), added, "from {}", dir.display());
    }
    let shown = env.mg(&["--json", "project", "show", "app"]);
    assert_eq!(shown.assert_code(0).json(), added);
    let listed = env.mg(&["--json", "project", "show"]).assert_code(0).json();
    assert_eq!(listed, json!({"projects": [added]}));

    let text = env.mg(&["project", "show", "app"]).assert_code(0).stdout;
    assert_eq!(
        text.trim_end(),
        format!(
            "app\n  backend   local\n  target    main\n  upstream  {}",
            path(repo.root())
        )
    );

    let doctor = env.mg(&["--json", "doctor"]).assert_code(0).json();
    assert_eq!(check(&doctor, "project app")["status"], "ok");
}

#[test]
fn f1_local_asks_for_the_target_when_several_could_be_it() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.branch("master");
    repo.branch("develop");
    repo.checkout("master");

    let asked = env
        .mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(2)
        .json();
    let decision = &asked["needs_decision"];
    assert_eq!(decision["flag"], "--target");
    assert_eq!(decision["default"], "master");
    // Only the conventional names; develop is work in progress.
    assert_eq!(
        decision["options"],
        json!([{"value": "main"}, {"value": "master", "description": "checked out"}])
    );
    // Asking registered nothing.
    let listed = env.mg(&["--json", "project", "show"]).assert_code(0).json();
    assert_eq!(listed, json!({"projects": []}));

    let added = env
        .mg_at(
            repo.root(),
            &["--json", "project", "add", "--target", "master"],
        )
        .assert_code(0)
        .json();
    assert_eq!(added["target"], "master");
    assert_eq!(staged(&env, "app", "master"), repo.rev("master"));
}

#[test]
fn f1_local_refuses_what_it_cannot_register() {
    let env = env();
    env.start_hub();
    let add = |args: &[&str]| {
        let args = [&["--json", "project", "add"], args].concat();
        env.mg(&args).assert_code(1).json()
    };
    let repo = TestRepo::new("app");
    env.mg(&["project", "add", path(repo.root())])
        .assert_code(0);

    let again = add(&[path(&repo.worktree("feature"))]);
    assert_eq!(error_code(&again), "already_registered");
    assert_eq!(again["error"]["hint"], "mg project show app");

    let namesake = TestRepo::new("app");
    assert_eq!(error_code(&add(&[path(namesake.root())])), "name_taken");
    let renamed = env
        .mg(&[
            "--json",
            "project",
            "add",
            path(namesake.root()),
            "--name",
            "app-2",
        ])
        .assert_code(0)
        .json();
    assert_eq!(renamed["name"], "app-2");

    let web = TestRepo::new("web");
    let github = add(&[path(web.root()), "--backend", "github"]);
    assert_eq!(error_code(&github), "backend_unavailable");
    let branch = add(&[path(web.root()), "--target", "nope"]);
    assert_eq!(error_code(&branch), "unknown_branch");
    let name = add(&[path(web.root()), "--name", "Web App"]);
    assert_eq!(error_code(&name), "invalid_name");

    let blank = TestRepo::empty("blank");
    assert_eq!(error_code(&add(&[path(blank.root())])), "no_commits");
    let plain = tempfile::tempdir().unwrap();
    assert_eq!(error_code(&add(&[path(plain.path())])), "not_a_repo");
    let missing = add(&[path(&plain.path().join("missing"))]);
    assert_eq!(error_code(&missing), "invalid_path");

    let listed = env.mg(&["--json", "project", "show"]).assert_code(0).json();
    let names: Vec<&Value> = listed["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| &p["name"])
        .collect();
    assert_eq!(names, [&json!("app"), &json!("app-2")]);
}

#[test]
fn f15_changes_a_projects_settings() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["project", "add"]).assert_code(0);

    repo.git(["checkout", "--quiet", "-b", "develop"]);
    repo.commit("develop work");
    let set = env
        .mg(&["--json", "project", "set", "app", "target", "develop"])
        .assert_code(0)
        .json();
    assert_eq!(set["target"], "develop");
    assert_eq!(staged(&env, "app", "develop"), repo.rev("develop"));

    // The repo moved; doctor names the fix, and applying it passes.
    let moved = repo.sibling("app-moved");
    std::fs::rename(repo.root(), &moved).unwrap();
    let doctor = env.mg(&["--json", "doctor"]).assert_code(1).json();
    assert_eq!(doctor["ok"], false);
    let broken = check(&doctor, "project app");
    assert_eq!(broken["status"], "fail");
    assert_eq!(broken["fix"], "mg project set app upstream PATH");
    let set = env
        .mg_at(
            &moved,
            &["--json", "project", "set", "app", "upstream", "."],
        )
        .assert_code(0)
        .json();
    assert_eq!(set["upstream"], path(&moved));
    let doctor = env.mg(&["--json", "doctor"]).assert_code(0).json();
    assert_eq!(doctor["ok"], true);

    let set = |args: &[&str]| {
        let args = [&["--json", "project", "set"], args].concat();
        env.mg(&args).assert_code(1).json()
    };
    assert_eq!(
        error_code(&set(&["app", "colour", "red"])),
        "invalid_setting"
    );
    assert_eq!(
        error_code(&set(&["app", "backend", "svn"])),
        "invalid_setting"
    );
    assert_eq!(
        error_code(&set(&["app", "backend", "github"])),
        "backend_unavailable"
    );
    assert_eq!(
        error_code(&set(&["app", "target", "nope"])),
        "unknown_branch"
    );
    let unknown = set(&["nope", "target", "main"]);
    assert_eq!(error_code(&unknown), "project_not_found");
    // Names can't reach other routes.
    let sneaky = set(&["../projects", "target", "main"]);
    assert_eq!(error_code(&sneaky), "project_not_found");
}

#[test]
fn project_commands_need_a_running_hub() {
    let env = env();
    let run = env
        .mg(&["--json", "project", "show"])
        .assert_code(69)
        .json();
    assert_eq!(error_code(&run), "hub_unavailable");
    assert_eq!(run["error"]["hint"], "mg hub start");
}
