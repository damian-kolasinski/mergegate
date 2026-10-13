//! Running the hub: `mg hub start | stop | status` against real hub processes.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use serde_json::json;
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
