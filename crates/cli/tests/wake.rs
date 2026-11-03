//! A resumable harness: `mg submit --wake CMD` leaves a command the hub
//! runs when the item has events from the operator its agent hasn't read, at most once per
//! event and never while the agent waits.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use testkit::{Env, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn post(env: &Env, path: &str, body: &Value) {
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
}

fn review(env: &Env, summary: &str) {
    post(
        env,
        "/api/items/app%231/reviews",
        &json!({ "revision": 1, "verdict": "comment", "summary": summary }),
    );
}

/// A hub with the `app` project and a clone on `feature`, nothing submitted.
fn setup() -> (Env, TestRepo) {
    setup_in(env())
}

fn setup_in(env: Env) -> (Env, TestRepo) {
    env.start_hub();
    with_project(env)
}

/// [`setup_in`] for a hub the caller started.
fn with_project(env: Env) -> (Env, TestRepo) {
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    (env, repo)
}

/// A wake command that appends a line about itself to `log`.
fn logging(log: &Path) -> String {
    format!(
        "printf '%s %s %s %s %s\\n' {{item}} \"$MG_ITEM\" \"$MG_CURSOR\" \"$(pwd -P)\" \
         \"$MERGEGATE_HOME\" >> {}",
        log.display()
    )
}

fn log_path(env: &Env) -> PathBuf {
    env.home().join("woken.log")
}

fn lines(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The log once it has `n` lines.
#[track_caller]
fn await_lines(log: &Path, n: usize) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let lines = lines(log);
        if lines.len() >= n || Instant::now() > deadline {
            assert_eq!(lines.len(), n, "{lines:?}");
            return lines;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Long enough for a wake that shouldn't run to have run.
fn settle() {
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn f3_the_hub_wakes_the_agent_once_per_unread_event() {
    let (env, repo) = setup();
    let log = log_path(&env);
    env.mg_at(repo.root(), &["submit", "--wake", &logging(&log)])
        .assert_code(0);
    settle();
    assert_eq!(lines(&log), [] as [String; 0], "no events yet");

    review(&env, "First.");
    let worktree = repo.root().canonicalize().unwrap();
    let home = env.home().display();
    assert_eq!(
        await_lines(&log, 1),
        [format!("app#1 app#1 1 {} {home}", worktree.display())]
    );
    settle();
    assert_eq!(lines(&log).len(), 1);

    review(&env, "Second.");
    assert_eq!(await_lines(&log, 2)[1].split(' ').nth(2), Some("2"));

    // Each event wakes once, even across a restart.
    env.mg(&["hub", "stop"]).assert_code(0);
    env.start_hub();
    settle();
    assert_eq!(lines(&log).len(), 2);
}

#[test]
fn f3_reading_the_events_first_leaves_nothing_to_wake_for() {
    let (env, repo) = setup();
    let log = log_path(&env);
    env.mg_at(repo.root(), &["submit", "--wake", &logging(&log)])
        .assert_code(0);
    let wait = Command::new(env!("CARGO_BIN_EXE_mg"))
        .args(["wait", "app#1", "--timeout", "30"])
        .env("MERGEGATE_HOME", env.home())
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_PID")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    // Time for the wait to connect.
    settle();
    review(&env, "While you wait.");
    let output = wait.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("While you wait."));
    settle();
    assert_eq!(lines(&log), [] as [String; 0]);
}

#[test]
fn f3_an_empty_wake_removes_it() {
    let (env, repo) = setup();
    let log = log_path(&env);
    env.mg_at(repo.root(), &["submit", "--wake", &logging(&log)])
        .assert_code(0);
    // The same head adds no revision, but still updates the wake.
    let again = env
        .mg_at(repo.root(), &["--json", "submit", "--wake", ""])
        .assert_code(0)
        .json();
    assert_eq!(again["new_revision"], false);
    review(&env, "Nobody to wake.");
    settle();
    assert_eq!(lines(&log), [] as [String; 0]);
}

#[test]
fn f3_its_own_abandon_does_not_wake_the_agent() {
    let (env, repo) = setup();
    let log = log_path(&env);
    env.mg_at(repo.root(), &["submit", "--wake", &logging(&log)])
        .assert_code(0);
    env.mg(&["abandon", "app#1"]).assert_code(0);
    settle();
    assert_eq!(lines(&log), [] as [String; 0]);
}

#[test]
fn f3_a_wake_commands_stderr_goes_to_a_log_in_the_home() {
    let (env, repo) = setup();
    env.mg_at(
        repo.root(),
        &["submit", "--wake", "echo 'no session' >&2; exit 3"],
    )
    .assert_code(0);
    review(&env, "First.");
    let log = env.home().join("wakes").join("app#1.log");
    let deadline = Instant::now() + Duration::from_secs(10);
    while std::fs::read_to_string(&log).unwrap_or_default().is_empty() {
        assert!(Instant::now() < deadline, "nothing in {}", log.display());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "no session\n");
}

#[test]
fn f3_the_wake_runs_outside_the_agent_session_that_started_the_hub() {
    // A hub started from an agent's shell inherits its session's markers,
    // and a harness that sees them won't start a session of its own.
    let (env, repo) = setup_in(env().with_vars(vec![
        ("CLAUDECODE".into(), "1".into()),
        ("CODEX_THREAD_ID".into(), "t".into()),
        ("CODEX_HOME".into(), "/codex".into()),
    ]));
    let log = log_path(&env);
    let wake = format!(
        "printf '%s %s %s\\n' \"${{CLAUDECODE-unset}}\" \"${{CODEX_THREAD_ID-unset}}\" \
         \"${{CODEX_HOME-unset}}\" >> {}",
        log.display()
    );
    env.mg_at(repo.root(), &["submit", "--wake", &wake])
        .assert_code(0);
    review(&env, "First.");
    assert_eq!(await_lines(&log, 1), ["unset unset /codex"]);
}

#[test]
fn f3_the_wakes_mg_is_the_hubs_whatever_mg_came_first_on_its_path() {
    // As macOS's /usr/bin/mg editor does on a PATH listing /usr/bin early.
    let editor = tempfile::tempdir().unwrap();
    let fake = editor.path().join("mg");
    std::fs::write(&fake, "#!/bin/sh\necho editor\n").unwrap();
    std::fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(editor.path().to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let env = env();
    env.mg_with_env(
        &["--json", "hub", "start", "--port", "0"],
        &[("PATH", path.to_str().unwrap())],
    )
    .assert_code(0);
    let (env, repo) = with_project(env);
    let log = log_path(&env);
    let wake = format!("command -v mg >> {}", log.display());
    env.mg_at(repo.root(), &["submit", "--wake", &wake])
        .assert_code(0);
    review(&env, "First.");
    let found = PathBuf::from(&await_lines(&log, 1)[0]);
    assert_eq!(
        found.canonicalize().unwrap(),
        Path::new(env!("CARGO_BIN_EXE_mg")).canonicalize().unwrap()
    );
}

struct Hook(Option<std::process::Child>);

impl Hook {
    fn start(env: &Env, repo: &TestRepo, session: &str) -> Self {
        Self::spawn(env, repo, session, None)
    }

    /// A hook that reports a lost hub after `ms`, not after a restart's time.
    fn reporting_a_lost_hub_after(env: &Env, repo: &TestRepo, session: &str, ms: &str) -> Self {
        Self::spawn(env, repo, session, Some(ms))
    }

    fn spawn(env: &Env, repo: &TestRepo, session: &str, reconnect_ms: Option<&str>) -> Self {
        use std::io::Write;
        let mut command = Command::new(env!("CARGO_BIN_EXE_mg"));
        command
            .args(["skill", "claude-watch"])
            .current_dir(repo.root())
            .env("MERGEGATE_HOME", env.home())
            .env_remove("MG_JSON")
            .env_remove("CLAUDE_PID")
            .env_remove("CODEX_THREAD_ID")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match reconnect_ms {
            Some(ms) => command.env("MG_WATCH_RECONNECT_MS", ms),
            None => command.env_remove("MG_WATCH_RECONNECT_MS"),
        };
        let mut child = command.spawn().unwrap();
        writeln!(
            child.stdin.take().unwrap(),
            "{}",
            json!({"session_id": session, "cwd": repo.root()})
        )
        .unwrap();
        Self(Some(child))
    }

    fn running(&mut self) {
        assert!(
            self.0.as_mut().unwrap().try_wait().unwrap().is_none(),
            "hook exited early"
        );
    }

    fn finish(mut self, code: i32) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "hook did not finish");
            std::thread::sleep(Duration::from_millis(20));
        }
        let output = self.0.take().unwrap().wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

impl Drop for Hook {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn armed(env: &Env, session: &str) {
    let path = env
        .home()
        .join("claude-hooks")
        .join(format!("{session}.json"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        assert!(Instant::now() < deadline, "hook did not arm");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn claude_hook_rearms_without_redelivering_and_preserves_feedback() {
    let (env, repo) = setup();
    env.mg_at(repo.root(), &["submit", "--wake", ""])
        .assert_code(0);
    let mut hook = Hook::start(&env, &repo, "session-review");
    armed(&env, "session-review");
    hook.running();
    let addr = env.hub_addr();
    let response = http_reply(
        &addr,
        &format!("GET /api/items/app%231 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(response.status, 200);
    let item: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(item["agent"], "waiting");
    assert!(
        Hook::start(&env, &repo, "session-review")
            .finish(0)
            .is_empty(),
        "duplicate hook is silent"
    );
    review(&env, "Fix the first issue.");
    assert!(hook.finish(2).contains("mg feedback app#1"));

    let mut hook = Hook::start(&env, &repo, "session-review");
    settle();
    hook.running();
    review(&env, "Fix the second issue.");
    assert!(hook.finish(2).contains("mg feedback app#1"));
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    assert_eq!(
        feedback["events"].as_array().unwrap().len(),
        2,
        "the hook acknowledged nothing"
    );
}

#[test]
fn claude_hook_and_hub_wakes_leave_a_successful_merge_silent() {
    for adapter in ["claude", "hub"] {
        let (env, repo) = setup();
        let log = log_path(&env);
        let wake = if adapter == "hub" {
            logging(&log)
        } else {
            String::new()
        };
        env.mg_at(repo.root(), &["submit", "--wake", &wake])
            .assert_code(0);
        let hook = if adapter == "claude" {
            let hook = Hook::start(&env, &repo, "session-merge");
            armed(&env, "session-merge");
            Some(hook)
        } else {
            None
        };
        post(
            &env,
            "/api/items/app%231/approve",
            &json!({"revision": 1, "summary": ""}),
        );
        if let Some(hook) = hook {
            assert!(hook.finish(0).is_empty());
        }
        settle();
        assert!(lines(&log).is_empty());
        let feedback = env
            .mg(&["--json", "feedback", "app#1"])
            .assert_code(0)
            .json();
        assert_eq!(feedback["state"], "merged");
        assert_eq!(feedback["events"][0]["verdict"], "approved");
        assert_eq!(feedback["events"][1]["type"], "item.merged");
        assert!(
            !feedback["events"][1]["commands"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn claude_hook_and_hub_wakes_report_merge_conflicts() {
    for adapter in ["claude", "hub"] {
        let (env, repo) = setup_in(env().with_vars(vec![("MG_TARGET_WATCH".into(), "off".into())]));
        let log = log_path(&env);
        let wake = if adapter == "hub" {
            logging(&log)
        } else {
            String::new()
        };
        env.mg_at(repo.root(), &["submit", "--wake", &wake])
            .assert_code(0);
        repo.checkout("main");
        repo.commit_files(
            "Conflicting change",
            &[("src/login.rs", "fn login(x: u8) {}\n")],
        );
        repo.checkout("feature");
        let hook = if adapter == "claude" {
            let hook = Hook::start(&env, &repo, "session-conflict");
            armed(&env, "session-conflict");
            Some(hook)
        } else {
            None
        };
        post(
            &env,
            "/api/items/app%231/approve",
            &json!({"revision": 1, "summary": ""}),
        );
        if let Some(hook) = hook {
            assert!(hook.finish(2).contains("mg feedback app#1"));
        } else {
            assert_eq!(await_lines(&log, 1)[0].split(' ').nth(2), Some("2"));
        }
        let feedback = env
            .mg(&["--json", "feedback", "app#1"])
            .assert_code(0)
            .json();
        assert_eq!(feedback["events"][1]["type"], "merge.conflict");
    }
}

#[test]
fn claude_hook_does_not_take_over_an_explicit_wake() {
    let (env, repo) = setup();
    let log = log_path(&env);
    env.mg_at(repo.root(), &["submit", "--wake", &logging(&log)])
        .assert_code(0);
    assert!(
        Hook::start(&env, &repo, "session-other")
            .finish(0)
            .is_empty()
    );
    review(&env, "Still use the registered wake.");
    await_lines(&log, 1);
}

#[test]
fn claude_hook_wakes_once_on_a_watcher_failure_then_reconnects() {
    let (env, repo) = setup();
    env.mg_at(repo.root(), &["submit", "--wake", ""])
        .assert_code(0);
    let hook = Hook::reporting_a_lost_hub_after(&env, &repo, "session-failure", "0");
    armed(&env, "session-failure");
    env.mg(&["hub", "stop"]).assert_code(0);
    assert!(hook.finish(2).contains("hub"));
    let mut hook = Hook::start(&env, &repo, "session-failure");
    settle();
    hook.running();
    env.start_hub();
    review(&env, "After reconnect.");
    assert!(hook.finish(2).contains("mg feedback app#1"));
}

#[test]
fn claude_hook_rides_out_a_hub_restart_without_a_turn() {
    let (env, repo) = setup();
    env.mg_at(repo.root(), &["submit", "--wake", ""])
        .assert_code(0);
    let mut hook = Hook::start(&env, &repo, "session-restart");
    armed(&env, "session-restart");
    env.mg(&["hub", "stop"]).assert_code(0);
    settle();
    hook.running();
    env.start_hub();
    review(&env, "After the restart.");
    assert!(hook.finish(2).contains("mg feedback app#1"));
}

#[test]
fn claude_hook_discovers_an_abandonment_before_its_first_stop() {
    let (env, repo) = setup();
    env.mg_at(repo.root(), &["submit", "--wake", ""])
        .assert_code(0);
    post(
        &env,
        "/api/items/app%231/abandon",
        &json!({"source": "operator", "reason": "Stop this work."}),
    );
    assert!(
        Hook::start(&env, &repo, "session-abandoned")
            .finish(2)
            .contains("mg feedback app#1")
    );
    assert!(
        Hook::start(&env, &repo, "session-abandoned")
            .finish(0)
            .is_empty()
    );
    let feedback = env
        .mg(&["--json", "feedback", "app#1"])
        .assert_code(0)
        .json();
    assert_eq!(feedback["events"][0]["type"], "item.abandoned");
}

#[test]
fn claude_hook_remembers_owned_items_across_turns() {
    let (env, repo) = setup();
    env.mg_at(repo.root(), &["submit", "--wake", ""])
        .assert_code(0);
    let hook = Hook::start(&env, &repo, "session-between-turns");
    armed(&env, "session-between-turns");
    review(&env, "Review first.");
    assert!(hook.finish(2).contains("mg feedback app#1"));
    let checkpoint: Value = serde_json::from_slice(
        &std::fs::read(env.home().join("claude-hooks/session-between-turns.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(checkpoint["known"], json!(["app#1"]));
    post(
        &env,
        "/api/items/app%231/abandon",
        &json!({"source": "operator", "reason": "Stop this work."}),
    );
    assert!(
        Hook::start(&env, &repo, "session-between-turns")
            .finish(2)
            .contains("mg feedback app#1")
    );
}
