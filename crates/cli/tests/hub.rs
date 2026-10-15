//! Running the hub: `mg hub start | stop | status` against real hub processes.

use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};
use testkit::{Env, http};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

#[test]
fn start_status_stop() {
    let env = env();
    let home = env.home().to_str().unwrap();

    let started = env.start_hub();
    assert_eq!(started["running"], true);
    assert_eq!(started["started"], true);
    assert_eq!(started["compatible"], true);
    assert_eq!(started["home"], home);
    let url = format!("http://{}", env.hub_addr());
    assert_eq!(started["url"], url.as_str());

    let status = env.mg(&["--json", "hub", "status"]).assert_code(0).json();
    assert_eq!(status["running"], true);
    assert_eq!(status["pid"], started["pid"]);
    assert_eq!(status["schema_version"], hub::SCHEMA_VERSION);

    let stopped = env.mg(&["--json", "hub", "stop"]).assert_code(0).json();
    assert_eq!(
        stopped,
        json!({"running": false, "stopped": true, "home": home})
    );
    assert!(!env.home().join("hub.json").exists());

    let status = env.mg(&["--json", "hub", "status"]).assert_code(0).json();
    assert_eq!(status, json!({"running": false, "home": home}));
}

/// A hub process paused with SIGSTOP, resumed when dropped so the test's
/// `Env` can stop it even after a failed assertion.
struct Paused(String);

impl Paused {
    fn new(pid: u64) -> Self {
        let pid = pid.to_string();
        assert!(
            Command::new("kill")
                .args(["-STOP", &pid])
                .status()
                .unwrap()
                .success()
        );
        Self(pid)
    }
}

impl Drop for Paused {
    fn drop(&mut self) {
        let _ = Command::new("kill").args(["-CONT", &self.0]).status();
    }
}

#[test]
fn a_hub_that_does_not_answer_is_reported_as_such_not_as_stopped() {
    let env = env();
    let pid = env.start_hub()["pid"].as_u64().unwrap();
    let _paused = Paused::new(pid);

    let run = env.mg(&["--json", "inbox"]).assert_code(69);
    let error = &run.json()["error"];
    assert_eq!(error["code"], "hub_unresponsive", "{run:?}");
    assert_eq!(
        error["hint"],
        format!("kill {pid}, then mg hub start"),
        "{run:?}"
    );
    let status = env.mg(&["--json", "hub", "status"]).assert_code(0).json();
    assert_eq!(status["unresponsive"], true);
}

/// Takes `env`'s hub lock, as a starting hub does before it answers.
fn hold_lock(env: &Env) -> File {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(env.home().join("hub.lock"))
        .unwrap();
    file.lock().unwrap();
    file
}

#[test]
fn a_command_waits_for_a_hub_that_is_starting() {
    let other = env();
    other.start_hub();
    let env = env();
    let lock = hold_lock(&env);
    let inbox = env.spawn(&["--json", "inbox"]);
    std::thread::sleep(Duration::from_millis(500));
    // A hub writes hub.json once it answers; another home's hub stands in.
    let info = env.home().join("hub.json");
    std::fs::copy(other.home().join("hub.json"), &info).unwrap();
    let inbox = inbox.wait_with_output().unwrap();
    // So this home's teardown leaves the other's hub to its own.
    std::fs::remove_file(&info).unwrap();
    drop(lock);
    let stdout = String::from_utf8_lossy(&inbox.stdout);
    assert_eq!(inbox.status.code(), Some(0), "{stdout}");
}

#[test]
fn a_hub_that_never_answers_names_no_pid_that_is_gone() {
    let env = env();
    let _lock = hold_lock(&env);
    let mut gone = Command::new("true").spawn().unwrap();
    let pid = gone.id();
    gone.wait().unwrap();
    let info = json!({"addr": "127.0.0.1:9", "pid": pid});
    std::fs::write(env.home().join("hub.json"), info.to_string()).unwrap();

    let run = env.mg(&["--json", "inbox"]).assert_code(69);
    let error = &run.json()["error"];
    assert_eq!(error["code"], "hub_unresponsive", "{run:?}");
    let hint = error["hint"].as_str().unwrap();
    assert!(!hint.contains(&pid.to_string()), "{hint}");
    assert!(hint.contains("hub.lock"), "{hint}");
    let status = env.mg(&["--json", "hub", "status"]).assert_code(0).json();
    assert_eq!(status["unresponsive"], true);
    assert_eq!(status["pid"], Value::Null);
}

#[test]
fn start_is_a_no_op_while_the_hub_runs() {
    let env = env();
    let first = env.start_hub();
    let second = env.start_hub();
    assert_eq!(second["started"], false);
    assert_eq!(second["pid"], first["pid"]);
    assert_eq!(second["url"], first["url"]);
}

#[test]
fn concurrent_starts_share_one_hub() {
    let env = env();
    let runs: Vec<_> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..4)
            .map(|_| s.spawn(|| env.mg(&["--json", "hub", "start", "--port", "0"])))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let outcomes: Vec<_> = runs.into_iter().map(|r| r.assert_code(0).json()).collect();
    let started = outcomes.iter().filter(|o| o["started"] == true).count();
    assert_eq!(started, 1, "{outcomes:?}");
    assert!(outcomes.iter().all(|o| o["pid"] == outcomes[0]["pid"]));

    // No losing start lingers to take over the home once the hub stops.
    env.mg(&["hub", "stop"]).assert_code(0);
    std::thread::sleep(Duration::from_millis(600));
    let status = env.mg(&["--json", "hub", "status"]).json();
    assert_eq!(status["running"], false, "{status}");
}

/// One leaked into `mg`, as a parent spawning on several threads can on
/// macOS, would keep that parent waiting for its end while the hub runs.
#[test]
#[allow(unsafe_code)]
fn the_hub_holds_no_descriptor_its_starter_inherited() {
    let env = env();
    let (mut ours, theirs) = UnixStream::pair().unwrap();
    ours.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    // SAFETY: clears close-on-exec on a descriptor this test owns, so the
    // `mg` it runs inherits it.
    unsafe { libc::fcntl(theirs.as_raw_fd(), libc::F_SETFD, 0) };
    env.start_hub();
    drop(theirs);
    let read = ours.read(&mut [0]);
    assert!(matches!(read, Ok(0)), "still open in the hub: {read:?}");
}

#[test]
fn a_home_reached_through_a_symlink_finds_its_hub() {
    let env = env();
    let started = env.start_hub();
    let links = tempfile::tempdir().unwrap();
    let link = links.path().join("home");
    std::os::unix::fs::symlink(env.home(), &link).unwrap();

    let via_link = [("MERGEGATE_HOME", link.to_str().unwrap())];
    let status = env
        .mg_with_env(&["--json", "hub", "status"], &via_link)
        .assert_code(0)
        .json();
    assert_eq!(status["running"], true);
    assert_eq!(status["pid"], started["pid"]);
    let again = env
        .mg_with_env(&["--json", "hub", "start", "--port", "0"], &via_link)
        .assert_code(0)
        .json();
    assert_eq!(again["started"], false);
}

/// Asks the hub to stop while a half-sent request keeps it draining for
/// `drain`, and returns the thread that ends that request.
fn stop_slowly(env: &Env, drain: Duration) -> std::thread::JoinHandle<()> {
    let addr = env.hub_addr();
    let mut pending = TcpStream::connect(&addr).unwrap();
    pending.write_all(b"GET /api/health HTTP/1.1\r\n").unwrap();
    let (status, body) = http(
        &addr,
        &format!(
            "POST /api/hub/shutdown HTTP/1.1\r\nHost: {addr}\r\nx-mergegate-client: test\r\n\
             Content-Length: 0\r\nConnection: close\r\n\r\n"
        ),
    );
    assert_eq!(status, 202, "{body}");
    std::thread::spawn(move || {
        std::thread::sleep(drain);
        drop(pending);
    })
}

#[test]
fn start_waits_out_a_hub_that_is_stopping() {
    let env = env();
    let old = env.start_hub();
    let drained = stop_slowly(&env, Duration::from_millis(500));
    let new = env.start_hub();
    assert_eq!(new["started"], true, "{new}");
    assert_ne!(new["pid"], old["pid"]);
    drained.join().unwrap();
}

#[test]
fn stop_waits_out_a_hub_that_is_stopping() {
    let env = env();
    env.start_hub();
    let drained = stop_slowly(&env, Duration::from_millis(500));
    let run = env.mg(&["--json", "hub", "stop"]).assert_code(0);
    assert_eq!(run.json()["stopped"], false);
    assert!(!env.home().join("hub.json").exists());
    drained.join().unwrap();
}

#[test]
fn stop_is_a_no_op_without_a_hub() {
    let env = env();
    let run = env.mg(&["hub", "stop"]).assert_code(0);
    assert_eq!(run.stdout.trim(), "hub not running");
}

#[test]
fn a_hub_with_its_own_home_never_gets_a_service() {
    let env = env();
    // Where launchd and systemd units would go, were MERGEGATE_HOME unset.
    let user = tempfile::tempdir().unwrap();
    let config = user.path().join(".config");
    let vars = [
        ("HOME", user.path().to_str().unwrap()),
        ("XDG_CONFIG_HOME", config.to_str().unwrap()),
    ];
    let started = env
        .mg_with_env(&["--json", "hub", "start", "--port", "0"], &vars)
        .assert_code(0)
        .json();
    assert_eq!(started["started"], true);
    assert!(started.get("service").is_none(), "{started}");
    assert!(started.get("warnings").is_none(), "{started}");
    let stopped = env
        .mg_with_env(&["--json", "hub", "stop"], &vars)
        .assert_code(0)
        .json();
    assert!(stopped.get("service_removed").is_none(), "{stopped}");
    assert!(!user.path().join("Library/LaunchAgents").exists());
    assert!(!config.join("systemd").exists());
}

#[test]
fn hub_survives_the_command_that_started_it() {
    let env = env();
    env.start_hub();
    // `mg hub start` has exited; its hub still answers.
    let (status, body) = http(
        &env.hub_addr(),
        &format!(
            "GET /api/health HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            env.hub_addr()
        ),
    );
    assert_eq!(status, 200, "{body}");
}

#[test]
fn one_hub_per_home() {
    let env = env();
    env.start_hub();
    let run = env
        .mg(&["--json", "hub", "start", "--foreground", "--port", "0"])
        .assert_code(1);
    assert_eq!(run.json()["error"]["code"], "hub_locked");
}

#[test]
fn homes_do_not_share_a_hub() {
    let (a, b) = (env(), env());
    let hub_a = a.start_hub();
    let hub_b = b.start_hub();
    assert_eq!(hub_b["started"], true);
    assert_ne!(hub_a["pid"], hub_b["pid"]);
    a.mg(&["hub", "stop"]).assert_code(0);
    let still = b.mg(&["--json", "hub", "status"]).json();
    assert_eq!(still["running"], true);
}

#[test]
fn a_taken_port_is_reported() {
    let env = env();
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port().to_string();
    let run = env
        .mg(&["--json", "hub", "start", "--port", &port])
        .assert_code(1);
    let error = &run.json()["error"];
    assert_eq!(error["code"], "port_in_use");
    assert_eq!(error["hint"], "mg hub start --port N");
}

#[test]
fn refuses_a_database_from_a_newer_mg() {
    let env = env();
    env.start_hub();
    env.mg(&["hub", "stop"]).assert_code(0);
    env.set_schema_version(99);

    let run = env
        .mg(&["--json", "hub", "start", "--port", "0"])
        .assert_code(1);
    assert_eq!(run.json()["error"]["code"], "schema_too_new");
    let status = env.mg(&["--json", "hub", "status"]).json();
    assert_eq!(status["running"], false);
}

#[test]
fn rejects_requests_not_addressed_to_loopback() {
    let env = env();
    env.start_hub();
    let addr = env.hub_addr();
    let (status, body) = http(
        &addr,
        "GET /api/health HTTP/1.1\r\nHost: attacker.example:80\r\nConnection: close\r\n\r\n",
    );
    assert_eq!(status, 403, "{body}");
}

#[test]
fn rejects_mutating_requests_without_the_client_header() {
    let env = env();
    env.start_hub();
    let addr = env.hub_addr();
    let (status, body) = http(
        &addr,
        &format!(
            "POST /api/hub/shutdown HTTP/1.1\r\nHost: {addr}\r\nContent-Length: 0\r\n\
             Connection: close\r\n\r\n"
        ),
    );
    assert_eq!(status, 403, "{body}");
    let status = env.mg(&["--json", "hub", "status"]).json();
    assert_eq!(status["running"], true);
}

#[test]
fn malformed_requests_are_answered_in_the_error_envelope() {
    let env = env();
    env.start_hub();
    let addr = env.hub_addr();
    let send = |request_line: &str, body: &str| {
        http(
            &addr,
            &format!(
                "{request_line} HTTP/1.1\r\nHost: {addr}\r\n{}: test\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                api::CLIENT_HEADER,
                body.len()
            ),
        )
    };
    for (request_line, body, expected_status, expected_code) in [
        ("POST /api/projects", "{}", 422, "invalid_request"),
        ("POST /api/projects", "not json", 400, "invalid_request"),
        ("GET /api/project-at", "", 400, "invalid_request"),
        ("GET /api/projects/", "", 404, "not_found"),
        ("DELETE /api/projects", "", 405, "method_not_allowed"),
    ] {
        let (status, reply) = send(request_line, body);
        assert_eq!(status, expected_status, "{request_line}: {reply}");
        let reply: serde_json::Value = serde_json::from_str(&reply)
            .unwrap_or_else(|e| panic!("{request_line}: not JSON ({e}): {reply}"));
        assert_eq!(reply["error"]["code"], expected_code, "{request_line}");
    }
}
