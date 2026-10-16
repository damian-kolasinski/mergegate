//! A hub left running across an `mg` upgrade: refused when the wire changed,
//! flagged when only the `mg` behind it did.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;

use serde_json::json;
use testkit::Env;

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

/// A hub that answers every request with the health an older `mg` would,
/// named in `env`'s `hub.json` under this test's pid, which its health
/// repeats. Declared after `env`, it drops first and takes `hub.json` along,
/// so `Env`'s cleanup has no hub to stop and no pid to kill: this test's own.
struct OldHub(PathBuf);

impl OldHub {
    fn new(env: &Env, version: &str, api_version: u32) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let pid = std::process::id();
        let health = json!({
            "version": version,
            "api_version": api_version,
            "schema_version": hub::SCHEMA_VERSION,
            "pid": pid,
            "home": env.home(),
        })
        .to_string();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let _ = stream.read(&mut [0; 4096]);
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{health}",
                    health.len()
                );
            }
        });
        let info = env.home().join("hub.json");
        std::fs::write(&info, json!({"addr": addr, "pid": pid}).to_string()).unwrap();
        Self(info)
    }
}

impl Drop for OldHub {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn a_hub_on_another_api_is_refused_until_restarted() {
    let env = env();
    let _hub = OldHub::new(&env, "0.0.1", api::API_VERSION.wrapping_add(1));

    let run = env.mg(&["--json", "inbox"]).assert_code(69);
    let error = &run.json()["error"];
    assert_eq!(error["code"], "hub_incompatible", "{run:?}");
    assert_eq!(error["hint"], "mg hub stop && mg hub start");

    let run = env.mg(&["--json", "hub", "start"]).assert_code(1);
    assert_eq!(run.json()["error"]["code"], "hub_incompatible", "{run:?}");

    let status = env.mg(&["--json", "hub", "status"]).assert_code(0).json();
    assert_eq!(status["compatible"], false);
}

#[test]
fn a_hub_running_another_mg_on_the_same_api_is_flagged() {
    let env = env();
    let _hub = OldHub::new(&env, "0.0.1+0123456789ab", api::API_VERSION);
    let warning = format!(
        "the hub runs mg 0.0.1+0123456789ab, this is mg {}; \
         mg hub stop && mg hub start runs this one",
        env!("MG_VERSION")
    );

    let status = env.mg(&["--json", "hub", "status"]).assert_code(0).json();
    assert_eq!(status["compatible"], true);
    assert_eq!(status["warnings"], json!([warning]));
    let text = env.mg(&["hub", "status"]).assert_code(0).stdout;
    assert!(text.contains(&format!("\nwarning: {warning}")), "{text}");

    let started = env.mg(&["--json", "hub", "start"]).assert_code(0).json();
    assert_eq!(started["started"], false);
    assert_eq!(started["warnings"], json!([warning]));
}

#[test]
fn a_hub_this_mg_started_is_current() {
    let env = env();
    let started = env.start_hub();
    assert_eq!(started["version"], env!("MG_VERSION"));
    assert_eq!(started["api_version"], api::API_VERSION);
    assert_eq!(started.get("warnings"), None, "{started}");
    let doctor = env.mg(&["--json", "doctor"]).json();
    assert_eq!(doctor["checks"][0]["name"], "hub");
    assert_eq!(doctor["checks"][0]["status"], "ok", "{doctor}");
}
