//! Opening an item for review: the hub serves each file's diff, computed
//! when the work was submitted, and the blobs that show more of a file.

use serde_json::{Value, json};
use testkit::{Env, Reply, TestRepo, http_reply};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn get(env: &Env, path: &str) -> Reply {
    let addr = env.hub_addr();
    http_reply(
        &addr,
        &format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    )
}

fn put(env: &Env, path: &str, body: &Value) -> Reply {
    let addr = env.hub_addr();
    let body = body.to_string();
    http_reply(
        &addr,
        &format!(
            "PUT {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
             {}: ui\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            api::CLIENT_HEADER,
            body.len(),
        ),
    )
}

fn json_of(reply: &Reply) -> Value {
    serde_json::from_str(&reply.body).unwrap_or_else(|e| panic!("{e}: {}", reply.body))
}

/// `app#1` modifies, adds, deletes and renames a file.
fn submitted() -> (Env, TestRepo) {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files(
        "Base",
        &[
            ("README", "title\nbody"),
            ("gone.txt", "bye\n"),
            ("src/old.rs", "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n"),
        ],
    );
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    for gone in ["gone.txt", "src/old.rs"] {
        std::fs::remove_file(repo.root().join(gone)).unwrap();
    }
    repo.commit_files(
        "Review me",
        &[
            ("README", "title\nbody\n"),
            ("logo.bin", "\0png"),
            ("src/new.rs", "fn a() {}\nfn b() {}\nfn c() {}\nfn e() {}\n"),
        ],
    );
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    (env, repo)
}

#[test]
fn f6_serves_an_items_files_with_their_diffs() {
    let (env, repo) = submitted();
    let blob = |rev: &str| repo.rev(rev);

    let reply = get(&env, "/api/items/app%231");
    assert_eq!(reply.status, 200, "{}", reply.body);
    let diff = json_of(&reply);
    assert_eq!(diff["item"]["id"], "app#1");
    assert_eq!(diff["item"]["revision"]["files"], 4);
    assert_eq!(
        diff["files"],
        json!([
            // With no guide and no judge, the heuristics rank: a changed
            // declaration first, docs last.
            {
                "path": "src/new.rs",
                "old_path": "src/old.rs",
                "old_blob": blob("main:src/old.rs"),
                "new_blob": blob("feature:src/new.rs"),
                "old_mode": "100644",
                "new_mode": "100644",
                "additions": 1,
                "deletions": 1,
                "content": {"kind": "text", "hunks": [{
                    "old_start": 1, "old_lines": 4, "new_start": 1, "new_lines": 4,
                    "lines": [" fn a() {}", " fn b() {}", " fn c() {}", "-fn d() {}", "+fn e() {}"],
                    "spans": [[], [], [], [], []],
                }], "lines_after": 0},
                "viewed": false,
                // No pack is installed; the header names the one that would
                // highlight it.
                "grammar": null,
                "missing_pack": "rust",
                "generated": false,
                "module": null,
                "tier": "review",
                "rule": null,
                "why": null,
                "reason": null,
                "interdiff": null,
                "importance": null,
                "judge_reason": null,
                "promoted": false,
                "raised": false,
                "hint": "ranked high: changes a declaration",
            },
            {
                "path": "README",
                "old_path": null,
                "old_blob": blob("main:README"),
                "new_blob": blob("feature:README"),
                "old_mode": "100644",
                "new_mode": "100644",
                "additions": 1,
                "deletions": 1,
                "content": {"kind": "text", "hunks": [{
                    "old_start": 1, "old_lines": 2, "new_start": 1, "new_lines": 2,
                    "lines": [" title", "-body", "\\ No newline at end of file", "+body"],
                    "spans": [[], [], [], []],
                }], "lines_after": 0},
                "viewed": false,
                "grammar": null,
                "missing_pack": null,
                "generated": false,
                "module": null,
                "tier": "review",
                "rule": null,
                "why": null,
                "reason": null,
                "interdiff": null,
                "importance": null,
                "judge_reason": null,
                "promoted": false,
                "raised": false,
                "hint": null,
            },
            {
                "path": "logo.bin",
                "old_path": null,
                "old_blob": null,
                "new_blob": blob("feature:logo.bin"),
                "old_mode": null,
                "new_mode": "100644",
                "additions": null,
                "deletions": null,
                "content": {"kind": "binary"},
                "viewed": false,
                "grammar": null,
                "missing_pack": null,
                "generated": false,
                "module": null,
                "tier": "review",
                "rule": null,
                "why": null,
                "reason": null,
                "interdiff": null,
                "importance": null,
                "judge_reason": null,
                "promoted": false,
                "raised": false,
                "hint": null,
            },
            {
                "path": "gone.txt",
                "old_path": null,
                "old_blob": blob("main:gone.txt"),
                "new_blob": null,
                "old_mode": "100644",
                "new_mode": null,
                "additions": 0,
                "deletions": 1,
                "content": {"kind": "text", "hunks": [{
                    "old_start": 1, "old_lines": 1, "new_start": 0, "new_lines": 0,
                    "lines": ["-bye"],
                    "spans": [[]],
                }], "lines_after": 0},
                "viewed": false,
                "grammar": null,
                "missing_pack": null,
                "generated": false,
                "module": null,
                "tier": "review",
                "rule": null,
                "why": null,
                "reason": null,
                "interdiff": null,
                "importance": null,
                "judge_reason": null,
                "promoted": false,
                "raised": false,
                "hint": "ranked low: docs",
            },
        ])
    );

    let missing = get(&env, "/api/items/app%232");
    assert_eq!(missing.status, 404, "{}", missing.body);
    assert_eq!(json_of(&missing)["error"]["code"], "item_not_found");
    let malformed = get(&env, "/api/items/app");
    assert_eq!(malformed.status, 400, "{}", malformed.body);
    assert_eq!(json_of(&malformed)["error"]["code"], "invalid_item");
}

#[test]
fn f6_serves_blobs_for_good() {
    let (env, repo) = submitted();
    let readme = repo.rev("feature:README");

    let reply = get(&env, &format!("/api/projects/app/blobs/{readme}"));
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(reply.body, "title\nbody\n");
    assert_eq!(
        reply.header("content-type"),
        Some("application/octet-stream")
    );
    assert_eq!(
        reply.header("cache-control"),
        Some("private, max-age=31536000, immutable")
    );
    assert_eq!(reply.header("x-content-type-options"), Some("nosniff"));

    // A commit is an object, but not a blob.
    for id in [repo.rev("feature"), "f".repeat(40), "nope".into()] {
        let reply = get(&env, &format!("/api/projects/app/blobs/{id}"));
        assert_eq!(reply.status, 404, "{id}: {}", reply.body);
        assert_eq!(json_of(&reply)["error"]["code"], "blob_not_found");
    }
    let unknown = get(&env, &format!("/api/projects/web/blobs/{readme}"));
    assert_eq!(unknown.status, 404, "{}", unknown.body);
    assert_eq!(json_of(&unknown)["error"]["code"], "project_not_found");
}

#[test]
fn f6_marks_files_viewed() {
    let (env, _repo) = submitted();
    let viewed = |env: &Env| -> Vec<String> {
        let diff = json_of(&get(env, "/api/items/app%231"));
        diff["files"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["viewed"] == true)
            .map(|f| f["path"].as_str().unwrap().to_owned())
            .collect()
    };
    let mark = |path: &str, viewed: bool| {
        let body = json!({"revision": 1, "path": path, "viewed": viewed});
        put(&env, "/api/items/app%231/viewed", &body)
    };

    for path in ["src/new.rs", "README"] {
        let reply = mark(path, true);
        assert_eq!(reply.status, 204, "{}", reply.body);
    }
    assert_eq!(viewed(&env), ["src/new.rs", "README"]);
    assert_eq!(mark("README", false).status, 204);
    assert_eq!(viewed(&env), ["src/new.rs"]);

    let unchanged = mark("src/old.rs", true);
    assert_eq!(unchanged.status, 404, "{}", unchanged.body);
    assert_eq!(json_of(&unchanged)["error"]["code"], "file_not_found");
    let body = json!({"revision": 2, "path": "README", "viewed": true});
    let later = put(&env, "/api/items/app%231/viewed", &body);
    assert_eq!(json_of(&later)["error"]["code"], "revision_not_found");
    let missing = put(&env, "/api/items/app%232/viewed", &body);
    assert_eq!(json_of(&missing)["error"]["code"], "item_not_found");
}

/// Each file of `diff` by path, with what shows its mode: both modes, the
/// content's kind and its hunk count.
fn modes(diff: &Value) -> Value {
    diff["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let path = f["path"].as_str().unwrap().to_owned();
            let hunks = f["content"]["hunks"].as_array().map_or(0, Vec::len);
            let shown = json!([f["old_mode"], f["new_mode"], f["content"]["kind"], hunks]);
            (path, shown)
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

#[test]
fn f6_shows_mode_and_type_changes() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("run.sh", "echo hi\n"), ("a.txt", "hello\n")]);
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.git(["update-index", "--chmod=+x", "run.sh"]);
    std::fs::remove_file(repo.root().join("a.txt")).unwrap();
    std::os::unix::fs::symlink("b", repo.root().join("a.txt")).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", repo.root().join("evil")).unwrap();
    // A gitlink needs no commit behind it, nor a .gitmodules.
    let pinned = repo.rev("main");
    repo.git([
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{pinned},vendor/dep"),
    ]);
    repo.git(["add", "a.txt", "evil"]);
    repo.commit("Modes");
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);

    let diff = json_of(&get(&env, "/api/items/app%231"));
    assert_eq!(
        modes(&diff),
        json!({
            // Only the mode changed: no lines, so the modes are all there is.
            "run.sh": ["100644", "100755", "text", 0],
            "a.txt": ["100644", "120000", "text", 1],
            "evil": [null, "120000", "text", 1],
            "vendor/dep": [null, "160000", "binary", 0],
        })
    );
    let dep = diff["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "vendor/dep")
        .unwrap();
    assert_eq!(dep["new_blob"], pinned);

    // A revision that only flips the bit back drops the change, and shows
    // the flip since the last one.
    repo.git(["update-index", "--chmod=-x", "run.sh"]);
    repo.commit("Not executable");
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    let since = json_of(&get(&env, "/api/items/app%231?since=1"));
    let run = since["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "run.sh")
        .unwrap();
    assert_eq!(run["interdiff"], "dropped");
    assert_eq!(
        json!([run["old_mode"], run["new_mode"]]),
        json!(["100755", "100644"])
    );
}
