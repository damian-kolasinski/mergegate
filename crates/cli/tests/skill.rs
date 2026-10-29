//! Setting agents up: `mg skill setup` puts a stub where each agent found
//! loads skills, pointing at `mg skill show`, and a session hook in those
//! that run one, whose `mg skill session-start` tells a session in a
//! mergegate clone how to work. `mg skill install` writes the whole skill
//! for agents setup doesn't know.

use std::fs;

use serde_json::Value;
use testkit::{Env, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

/// An env whose user has Claude Code and Codex installed.
fn with_agents() -> Env {
    let env = env();
    fs::create_dir(env.user_home().join(".claude")).unwrap();
    fs::create_dir(env.user_home().join(".codex")).unwrap();
    env
}

fn changes(report: &Value) -> Vec<(String, String, String)> {
    report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| {
            let agents: Vec<_> = step["agents"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap())
                .collect();
            (
                step["kind"].as_str().unwrap().to_owned(),
                agents.join(","),
                step["change"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn triple(kind: &str, agents: &str, change: &str) -> (String, String, String) {
    (kind.into(), agents.into(), change.into())
}

#[test]
fn setup_installs_the_stub_and_hook_into_the_agents_found() {
    let env = with_agents();
    let report = env.mg(&["--json", "skill", "setup"]).assert_code(0).json();
    assert_eq!(report["dry_run"], false);
    assert_eq!(
        changes(&report),
        [
            triple("skill", "claude", "installed"),
            triple("skill", "codex", "installed"),
            triple("hook", "claude", "installed"),
            triple("hook", "codex", "installed"),
        ]
    );
    assert_eq!(
        report["steps"][3]["approve"],
        "trust the new hook with /hooks in Codex"
    );
    let user = env.user_home();
    let stub = fs::read_to_string(user.join(".agents/skills/mergegate/SKILL.md")).unwrap();
    assert!(stub.starts_with("---\nname: mergegate\n"), "{stub}");
    assert!(stub.contains("Run `mg skill show`"), "{stub}");
    let settings: Value =
        serde_json::from_str(&fs::read_to_string(user.join(".claude/settings.json")).unwrap())
            .unwrap();
    assert_eq!(
        settings["hooks"]["SessionStart"][0]["hooks"][0]["command"],
        "/bin/sh -c 'mergegate skill session-start --agent claude 2>/dev/null || true'"
    );

    let status = env.mg(&["--json", "skill", "status"]).assert_code(0).json();
    let ready: Vec<_> = status["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["agent"].as_str().unwrap(),
                a["found"] == true,
                a["ready"] == true,
            )
        })
        .collect();
    // OpenCode reads the folder Codex does, and holds no hook.
    assert_eq!(
        ready,
        [
            ("claude", true, true),
            ("codex", true, true),
            ("opencode", false, true)
        ]
    );

    let again = env.mg(&["skill", "setup"]).assert_code(0).stdout;
    assert!(!again.contains("installed"), "{again}");
    assert!(
        again.contains("skill  unchanged    ~/.claude/skills/mergegate  (Claude Code)"),
        "{again}"
    );
}

#[test]
fn setup_finds_nothing_to_set_up_without_agents() {
    let env = env();
    let run = env.mg(&["skill", "setup"]).assert_code(0);
    assert!(
        run.stdout.starts_with("found no agent to set up"),
        "{run:?}"
    );
    assert!(!env.user_home().join(".claude").exists());
    // Named, it sets that agent up all the same.
    let named = env
        .mg(&["--json", "skill", "setup", "--agent", "opencode"])
        .assert_code(0)
        .json();
    assert_eq!(
        changes(&named),
        [
            triple("skill", "opencode", "installed"),
            triple("hook", "opencode", "unsupported"),
        ]
    );
}

#[test]
fn a_dry_run_says_what_setup_would_do() {
    let env = with_agents();
    let text = env
        .mg(&["skill", "setup", "--dry-run"])
        .assert_code(0)
        .stdout;
    assert!(text.starts_with("Dry run: nothing was changed."), "{text}");
    assert!(text.contains("installed"), "{text}");
    assert!(!env.user_home().join(".claude/skills").exists());
}

#[test]
fn setup_leaves_a_skill_it_did_not_write_and_exits_1() {
    let env = with_agents();
    let theirs = env.user_home().join(".claude/skills/mergegate");
    fs::create_dir_all(&theirs).unwrap();
    fs::write(theirs.join("SKILL.md"), "---\nname: theirs\n---\n").unwrap();
    let run = env
        .mg(&["skill", "setup", "--agent", "claude"])
        .assert_code(1);
    assert!(run.stdout.contains("conflict"), "{run:?}");
    assert!(run.stdout.contains("mg skill setup --force"), "{run:?}");
    env.mg(&["skill", "setup", "--agent", "claude", "--force"])
        .assert_code(0);
}

#[test]
fn remove_takes_out_only_what_setup_installed() {
    let env = with_agents();
    let settings = env.user_home().join(".claude/settings.json");
    fs::write(&settings, "{\n  \"model\": \"opus\"\n}\n").unwrap();
    env.mg(&["skill", "setup"]).assert_code(0);
    let report = env.mg(&["--json", "skill", "remove"]).assert_code(0).json();
    assert!(
        changes(&report)
            .iter()
            .all(|(_, _, change)| change == "removed"),
        "{report}"
    );
    assert_eq!(
        fs::read_to_string(&settings).unwrap(),
        "{\n  \"model\": \"opus\"\n}\n"
    );
    assert!(!env.user_home().join(".claude/skills/mergegate").exists());
}

#[test]
fn setup_for_a_project_writes_into_the_repository() {
    let env = with_agents();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    env.mg_at(repo.root(), &["skill", "setup", "--project"])
        .assert_code(0);
    assert!(
        repo.root()
            .join(".claude/skills/mergegate/SKILL.md")
            .is_file()
    );
    assert!(repo.root().join(".codex/hooks.json").is_file());
    assert!(!env.user_home().join(".claude/skills").exists());
}

#[test]
fn show_prints_the_guide_and_its_references() {
    let env = env();
    let guide = env.mg(&["skill", "show"]).assert_code(0).stdout;
    assert!(guide.starts_with("# mergegate\n"), "{guide}");
    assert!(guide.contains("`mg skill show waiting`"), "{guide}");
    assert!(!guide.contains("](references/"), "{guide}");
    let waiting = env.mg(&["skill", "show", "waiting"]).assert_code(0).stdout;
    assert!(waiting.starts_with("# Waiting"), "{waiting}");

    let unknown = env
        .mg(&["--json", "skill", "show", "nope"])
        .assert_code(1)
        .json();
    assert_eq!(unknown["error"]["code"], "unknown_reference");
    assert_eq!(
        unknown["error"]["message"],
        "the guide has no reference \"nope\"; it has setup, authoring, waiting, operator"
    );
}

#[test]
fn session_start_says_nothing_outside_a_mergegate_clone() {
    let env = env();
    let repo = TestRepo::new("other");
    repo.commit_files("Base", &[("README", "title\n")]);
    let run = env
        .mg_at(
            repo.root(),
            &["skill", "session-start", "--agent", "claude"],
        )
        .assert_code(0);
    assert_eq!(run.stdout, "");
}

#[test]
fn session_start_tells_a_session_its_item_and_how_to_wait() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    env.mg_at(repo.root(), &["project", "add"]).assert_code(0);
    env.mg_at(repo.root(), &["init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);

    let before = env
        .mg_at(
            repo.root(),
            &["skill", "session-start", "--agent", "claude"],
        )
        .assert_code(0)
        .stdout;
    assert!(
        before.starts_with("This repository goes through mergegate"),
        "{before}"
    );
    assert!(before.contains("as project app."), "{before}");
    assert!(!before.contains("is open on this branch"), "{before}");
    assert!(
        before.contains("`mg wait ITEM` run in the background"),
        "{before}"
    );
    // An agent would take JSON on a hook's stdout for its own control format.
    let json = env
        .mg_at(
            repo.root(),
            &["--json", "skill", "session-start", "--agent", "claude"],
        )
        .assert_code(0)
        .stdout;
    assert_eq!(json, before);

    env.mg_at(repo.root(), &["submit"]).assert_code(0);
    let after = env
        .mg_at(repo.root(), &["skill", "session-start", "--agent", "codex"])
        .assert_code(0)
        .stdout;
    assert!(
        after.contains(
            "app#1, \"Add login\", is open on this branch, waiting for review. If you are \
             picking it up, `mg resume app#1` brings you up to date."
        ),
        "{after}"
    );
    assert!(after.contains("mg wakes codex $CODEX_THREAD_ID"), "{after}");

    env.mg(&["hub", "stop"]).assert_code(0);
    let down = env
        .mg_at(
            repo.root(),
            &["skill", "session-start", "--agent", "claude"],
        )
        .assert_code(0)
        .stdout;
    assert!(down.contains("The hub didn't answer just now"), "{down}");
}

#[test]
fn the_hub_reports_and_sets_up_its_machines_agents() {
    let env = with_agents();
    env.start_hub();
    let addr = env.hub_addr();
    let status = http_reply(
        &addr,
        &format!("GET /api/agents HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(status.status, 200, "{}", status.body);
    let status: Value = serde_json::from_str(&status.body).unwrap();
    assert_eq!(status["agents"][0]["found"], true);
    assert_eq!(status["agents"][0]["ready"], false);
    assert_eq!(status["agents"][0]["skill"]["state"], "missing");

    let set_up = http_reply(
        &addr,
        &format!(
            "POST /api/agents/setup HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
             {}: ui\r\nContent-Length: 0\r\n\r\n",
            api::CLIENT_HEADER
        ),
    );
    assert_eq!(set_up.status, 200, "{}", set_up.body);
    let set_up: Value = serde_json::from_str(&set_up.body).unwrap();
    assert_eq!(set_up["steps"][0]["change"], "installed");
    assert!(
        env.user_home()
            .join(".claude/skills/mergegate/SKILL.md")
            .is_file()
    );
}

#[test]
fn install_writes_the_whole_skill_under_the_home() {
    let env = env();
    let installed = env
        .mg(&["--json", "skill", "install"])
        .assert_code(0)
        .json();
    let path = env.home().join("skill/mergegate");
    assert_eq!(installed["path"], path.display().to_string());
    let files: Vec<_> = installed["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        [
            "SKILL.md",
            "references/setup.md",
            "references/authoring.md",
            "references/waiting.md",
            "references/operator.md",
            // For the wakes Codex agents left before `mg wakes codex`.
            "scripts/codex_wake.py",
        ]
    );
    for (file, contents) in skill::FILES {
        assert_eq!(&fs::read_to_string(path.join(file)).unwrap(), contents);
    }
    assert!(
        installed["next"]
            .as_str()
            .unwrap()
            .contains("mg skill setup"),
        "{installed}"
    );
}

#[test]
fn installing_again_replaces_the_folder() {
    let env = env();
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path().to_str().unwrap();
    env.mg(&["skill", "install", "--dir", at]).assert_code(0);
    let stale = dir.path().join("mergegate/references/dropped.md");
    fs::write(&stale, "from an older mg").unwrap();

    let text = env.mg(&["skill", "install", "--dir", at]).assert_code(0);
    assert!(
        text.stdout.starts_with(&format!(
            "installed the mergegate skill in {at}/mergegate\n"
        )),
        "{text:?}"
    );
    assert!(!stale.exists());
    assert!(dir.path().join("mergegate/SKILL.md").is_file());
    let left: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(left.len(), 1, "no staging folder left behind");
}

#[test]
fn install_leaves_a_folder_it_did_not_write() {
    let env = env();
    let dir = tempfile::tempdir().unwrap();
    let theirs = dir.path().join("mergegate");
    fs::create_dir(&theirs).unwrap();
    fs::write(theirs.join("notes.md"), "mine").unwrap();
    let refused = env
        .mg(&[
            "--json",
            "skill",
            "install",
            "--dir",
            dir.path().to_str().unwrap(),
        ])
        .assert_code(1)
        .json();
    assert_eq!(refused["error"]["code"], "not_a_skill");
    assert!(theirs.join("notes.md").is_file());
}

#[test]
fn a_codex_wake_runs_the_script_mg_carries() {
    let env = env();
    // A codex that fails as it queues: the script ran, with the thread. The
    // system's python3, not a version manager's shim, which reads config
    // from the user's home.
    let failed = env
        .mg_with_env(
            &["--json", "wakes", "codex", "thread-1"],
            &[("CODEX_BIN", "/usr/bin/false"), ("PATH", "/usr/bin:/bin")],
        )
        .assert_code(1);
    assert_eq!(failed.json()["error"]["code"], "wake_failed");
    assert!(failed.stderr.contains("CalledProcessError"), "{failed:?}");
}
