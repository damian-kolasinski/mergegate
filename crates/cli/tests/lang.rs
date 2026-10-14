//! Language packs: installing one highlights the diffs of the files it
//! claims, in the background; removing it leaves them plain, naming the pack.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use testkit::{Env, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

/// A pack built by `langs/build.mjs`, kept small enough to commit.
fn json_pack() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../highlight/tests/fixtures/json-0.24.8.tar.gz")
}

fn json_file(env: &Env) -> Value {
    let addr = env.hub_addr();
    let reply = http_reply(
        &addr,
        &format!("GET /api/items/app%231 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    let diff: Value = serde_json::from_str(&reply.body).unwrap();
    diff["files"][0].clone()
}

/// The file once the highlight pass has run for it.
fn highlighted(env: &Env) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let file = json_file(env);
        if file["content"]["hunks"][0]["spans"][0] != json!([]) {
            return file;
        }
        assert!(Instant::now() < deadline, "never highlighted: {file}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn installed_packs_highlight_diffs() {
    let env = env();
    env.start_hub();
    let listed = env.mg(&["--json", "lang", "list"]).assert_code(0).json();
    assert_eq!(listed, json!({"packs": []}));

    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Config", &[("config.json", "{\"a\": 1}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);

    let plain = json_file(&env);
    assert_eq!(plain["grammar"], Value::Null);
    assert_eq!(plain["missing_pack"], "json");
    assert_eq!(plain["content"]["hunks"][0]["spans"], json!([[]]));

    let pack = json_pack();
    let added = env
        .mg(&["--json", "lang", "add", "--from", pack.to_str().unwrap()])
        .assert_code(0)
        .json();
    let installed = json!({"packs": [{
        "name": "json",
        "version": "0.24.8",
        "grammars": [{
            "name": "json",
            "extensions": ["json", "jsonc", "json5"],
            "filenames": [".prettierrc", ".babelrc", ".eslintrc"],
        }],
    }]});
    assert_eq!(added, installed);
    let listed = env.mg(&["--json", "lang", "list"]).assert_code(0).json();
    assert_eq!(listed, installed);
    let text = env.mg(&["lang", "list"]).assert_code(0).stdout;
    assert_eq!(
        text.trim_end(),
        "json  0.24.8  .json .jsonc .json5 .prettierrc .babelrc .eslintrc"
    );

    let file = highlighted(&env);
    assert_eq!(file["grammar"], "json");
    assert_eq!(file["missing_pack"], Value::Null);
    // `{"a": 1}`, columns counted without the `+`.
    assert_eq!(
        file["content"]["hunks"][0]["spans"],
        json!([[[1, 3, "string"], [6, 1, "number"]]])
    );

    let removed = env
        .mg(&["--json", "lang", "remove", "json"])
        .assert_code(0)
        .json();
    assert_eq!(removed, installed["packs"][0]);
    let plain = json_file(&env);
    assert_eq!(plain["missing_pack"], "json");
    assert_eq!(plain["content"]["hunks"][0]["spans"], json!([[]]));

    let missing = env
        .mg(&["--json", "lang", "remove", "json"])
        .assert_code(1)
        .json();
    assert_eq!(missing["error"]["code"], "pack_not_installed");
    assert_eq!(missing["error"]["hint"], "mg lang list");
}

#[test]
fn rejects_what_is_not_a_pack() {
    let env = env();
    env.start_hub();
    let empty = tempfile::tempdir().unwrap();
    let run = env
        .mg(&[
            "--json",
            "lang",
            "add",
            "--from",
            empty.path().to_str().unwrap(),
        ])
        .assert_code(1)
        .json();
    assert_eq!(run["error"]["code"], "no_packs");

    let junk = empty.path().join("junk.tar.gz");
    std::fs::write(&junk, "not a tarball").unwrap();
    let run = env
        .mg(&["--json", "lang", "add", "--from", junk.to_str().unwrap()])
        .assert_code(1)
        .json();
    assert_eq!(run["error"]["code"], "invalid_pack");
    let listed = env.mg(&["--json", "lang", "list"]).assert_code(0).json();
    assert_eq!(listed, json!({"packs": []}));
}
