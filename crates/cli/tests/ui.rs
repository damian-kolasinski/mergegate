//! The hub serves the UI next to its API: the bundle built into mg, or the
//! files in `MG_UI_DIR` while developing it.

use testkit::{Env, Reply, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn get(env: &Env, path: &str, extra: &str) -> Reply {
    let addr = env.hub_addr();
    http_reply(
        &addr,
        &format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\n{extra}Connection: close\r\n\r\n"),
    )
}

#[test]
fn the_hub_serves_the_built_ui() {
    let env = env();
    env.start_hub();

    let index = get(&env, "/", "");
    assert_eq!(index.status, 200, "{index:?}");
    assert_eq!(
        index.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    let policy = index.header("content-security-policy").unwrap();
    assert!(policy.starts_with("default-src 'self'"), "{policy}");
    assert!(index.body.contains("/assets/main.js"), "{}", index.body);

    let script = get(&env, "/assets/main.js", "");
    assert_eq!(script.status, 200);
    assert_eq!(
        script.header("content-type"),
        Some("text/javascript; charset=utf-8")
    );
    assert_eq!(script.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(script.header("cache-control"), Some("no-cache"));
    assert!(!script.body.is_empty());

    let etag = script.header("etag").unwrap();
    let again = get(
        &env,
        "/assets/main.js",
        &format!("If-None-Match: {etag}\r\n"),
    );
    assert_eq!(again.status, 304);
    assert!(again.body.is_empty());
}

#[test]
fn a_missing_asset_is_a_404_in_the_error_envelope() {
    let env = env();
    env.start_hub();
    let missing = get(&env, "/assets/nope.js", "");
    assert_eq!(missing.status, 404);
    let body: serde_json::Value = serde_json::from_str(&missing.body).unwrap();
    assert_eq!(body["error"]["code"], "not_found");
}

#[test]
fn mg_ui_dir_serves_the_files_on_disk() {
    let env = env();
    let site = tempfile::tempdir().unwrap();
    std::fs::create_dir(site.path().join("assets")).unwrap();
    std::fs::write(site.path().join("index.html"), "dev index").unwrap();
    std::fs::write(site.path().join("assets/main.js"), "dev script").unwrap();
    env.mg_with_env(
        &["hub", "start", "--port", "0"],
        &[("MG_UI_DIR", site.path().to_str().unwrap())],
    )
    .assert_code(0);

    assert_eq!(get(&env, "/", "").body, "dev index");
    // Read on every request: an edit shows without restarting the hub.
    std::fs::write(site.path().join("assets/main.js"), "edited").unwrap();
    let script = get(&env, "/assets/main.js", "");
    assert_eq!(script.body, "edited");
    assert_eq!(script.header("cache-control"), Some("no-store"));
    assert_eq!(get(&env, "/assets/../index.html", "").status, 404);

    let doctor = env.mg(&["--json", "doctor"]).assert_code(0).json();
    assert_eq!(doctor["checks"][1]["name"], "ui");
    let message = doctor["checks"][1]["message"].as_str().unwrap();
    assert!(message.ends_with("(MG_UI_DIR)"), "{message}");
}
