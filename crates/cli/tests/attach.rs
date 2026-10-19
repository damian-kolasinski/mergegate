//! Demo attachments: screenshots and recordings an agent adds with
//! `mg submit --attach` or `mg attach`, served to the review screen and
//! dropped a while after the item closes.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use testkit::{Env, Reply, TestRepo, http_bytes, http_reply};

/// All ASCII, so the raw HTTP reader can hold it as a string.
const GIF: &[u8] = b"GIF89a demo frame";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR shot";

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn error_code(run: &Value) -> &str {
    run["error"]["code"].as_str().unwrap_or_default()
}

/// A hub with `app` registered, its clone on branch `feature` with a commit.
fn hub_with_work() -> (Env, TestRepo) {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Add the parser", &[("src/parser.rs", "fn parse() {}\n")]);
    (env, repo)
}

fn file(repo: &TestRepo, name: &str, bytes: &[u8]) -> PathBuf {
    let path = repo.sibling(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn arg(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn get(env: &Env, path: &str, extra: &str) -> Reply {
    let addr = env.hub_addr();
    http_reply(
        &addr,
        &format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\n{extra}Connection: close\r\n\r\n"),
    )
}

fn item(env: &Env) -> Value {
    let reply = get(env, "/api/items/app%231", "");
    assert_eq!(reply.status, 200, "{reply:?}");
    serde_json::from_str(&reply.body).unwrap()
}

fn inbox(env: &Env) -> Value {
    env.mg(&["--json", "inbox"]).assert_code(0).json()
}

#[test]
fn f16_submit_attaches_files_to_the_revision_it_submits() {
    let (env, repo) = hub_with_work();
    let gif = file(&repo, "flow.gif", GIF);
    let png = file(&repo, "shot.png", PNG);

    let submitted = env
        .mg_at(
            repo.root(),
            &[
                "--json",
                "submit",
                "--attach",
                arg(&gif),
                "--attach",
                arg(&png),
            ],
        )
        .assert_code(0)
        .json();
    assert_eq!(submitted["item"]["id"], "app#1");
    let attachments = &submitted["attachments"];
    assert_eq!(attachments[0]["name"], "flow.gif");
    assert_eq!(attachments[0]["media_type"], "image/gif");
    assert_eq!(attachments[0]["video"], false);
    assert_eq!(attachments[0]["size"], GIF.len());
    assert_eq!(attachments[0]["revision"], 1);
    assert_eq!(attachments[1]["name"], "shot.png");
    assert_eq!(attachments[1]["media_type"], "image/png");

    let demo = &item(&env)["demo"];
    assert_eq!(demo["revision"], 1);
    assert_eq!(&demo["attachments"], attachments);
    assert_eq!(inbox(&env)["needs_you"][0]["demo"], true);

    // Without --attach the output has no attachments at all.
    repo.commit_files(
        "Parse numbers",
        &[("src/parser.rs", "fn parse() -> u32 { 0 }\n")],
    );
    let text = env.mg_at(repo.root(), &["submit"]).assert_code(0).stdout;
    assert!(!text.contains("attached"), "{text}");
    let again = env.mg_at(repo.root(), &["--json", "submit"]).json();
    assert_eq!(again.get("attachments"), None, "{again}");
}

#[test]
fn f16_a_file_that_is_not_media_stops_the_submit_before_it_pushes() {
    let (env, repo) = hub_with_work();
    let svg = file(
        &repo,
        "logo.svg",
        b"<svg xmlns='http://www.w3.org/2000/svg'/>",
    );
    let gif = file(&repo, "flow.gif", GIF);

    let run = env
        .mg_at(
            repo.root(),
            &[
                "--json",
                "submit",
                "--attach",
                arg(&gif),
                "--attach",
                arg(&svg),
            ],
        )
        .assert_code(1)
        .json();
    assert_eq!(error_code(&run), "unsupported_media", "{run}");
    assert_eq!(inbox(&env)["needs_you"], json!([]));
    let staging = env.home().join("projects/app/staging.git");
    let branches = testkit::git(&staging, ["branch", "--list", "feature"]);
    assert_eq!(branches.trim(), "", "nothing was pushed");

    let missing = env
        .mg_at(repo.root(), &["--json", "submit", "--attach", "nope.png"])
        .assert_code(1)
        .json();
    assert_eq!(error_code(&missing), "attachment_unreadable", "{missing}");
}

#[test]
fn f16_attach_adds_to_the_newest_revision_and_the_panel_falls_back() {
    let (env, repo) = hub_with_work();
    let gif = file(&repo, "flow.gif", GIF);
    env.mg_at(repo.root(), &["--json", "submit", "--attach", arg(&gif)])
        .assert_code(0);
    repo.commit_files(
        "Parse numbers",
        &[("src/parser.rs", "fn parse() -> u32 { 0 }\n")],
    );
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);

    // r2 has none, so the panel shows r1's.
    let demo = &item(&env)["demo"];
    assert_eq!(demo["revision"], 1);
    assert_eq!(demo["attachments"][0]["name"], "flow.gif");

    let png = file(&repo, "shot.png", PNG);
    let text = env
        .mg(&["attach", "app#1", arg(&png)])
        .assert_code(0)
        .stdout;
    assert_eq!(
        text.trim_end(),
        format!("attached shot.png to app#1 r2: image/png, {} B", PNG.len())
    );
    let demo = &item(&env)["demo"];
    assert_eq!(demo["revision"], 2);
    assert_eq!(demo["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(demo["attachments"][0]["name"], "shot.png");

    let run = env.mg(&["--json", "attach", "app#9", arg(&png)]).json();
    assert_eq!(error_code(&run), "item_not_found", "{run}");
}

#[test]
fn f16_the_hub_refuses_what_is_not_media_whatever_the_client() {
    let (env, repo) = hub_with_work();
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    let addr = env.hub_addr();
    let body = "<html><script>alert(1)</script></html>";
    let reply = http_reply(
        &addr,
        &format!(
            "POST /api/items/app%231/attachments?name=x.png HTTP/1.1\r\nHost: {addr}\r\n\
             x-mergegate-client: test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    );
    assert_eq!(reply.status, 415, "{reply:?}");
    assert!(reply.body.contains("unsupported_media"), "{}", reply.body);
    assert_eq!(item(&env)["demo"], Value::Null);
    assert_eq!(inbox(&env)["needs_you"][0]["demo"], false);
}

#[test]
fn f16_serves_attachments_by_hash_with_ranges() {
    let (env, repo) = hub_with_work();
    let gif = file(&repo, "flow.gif", GIF);
    let submitted = env
        .mg_at(repo.root(), &["--json", "submit", "--attach", arg(&gif)])
        .assert_code(0)
        .json();
    let url = submitted["attachments"][0]["url"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(url.starts_with("/api/attachments/"), "{url}");

    let whole = get(&env, &url, "");
    assert_eq!(whole.status, 200, "{whole:?}");
    assert_eq!(whole.body.as_bytes(), GIF);
    assert_eq!(whole.header("content-type"), Some("image/gif"));
    assert_eq!(whole.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(whole.header("content-security-policy"), Some("sandbox"));
    assert_eq!(whole.header("accept-ranges"), Some("bytes"));
    assert_eq!(
        whole.header("cache-control"),
        Some("private, max-age=31536000, immutable")
    );

    let part = get(&env, &url, "Range: bytes=0-3\r\n");
    assert_eq!(part.status, 206, "{part:?}");
    assert_eq!(part.body, "GIF8");
    let total = GIF.len();
    assert_eq!(
        part.header("content-range"),
        Some(format!("bytes 0-3/{total}").as_str())
    );
    let past = get(&env, &url, &format!("Range: bytes={total}-\r\n"));
    assert_eq!(past.status, 416, "{past:?}");

    let unknown = get(&env, &format!("/api/attachments/{}", "0".repeat(64)), "");
    assert_eq!(unknown.status, 404, "{unknown:?}");
    let bogus = get(&env, "/api/attachments/..%2Fhub.db", "");
    assert_eq!(bogus.status, 404, "{bogus:?}");
}

/// [`get`] for a body that may not be text, such as a zip's.
fn get_bytes(env: &Env, path: &str) -> Reply<Vec<u8>> {
    let addr = env.hub_addr();
    http_bytes(
        &addr,
        &format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    )
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn f16_serves_the_whole_demo_as_one_zip() {
    let (env, repo) = hub_with_work();
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);
    assert_eq!(item(&env)["demo"], Value::Null);
    let missing = get_bytes(&env, "/api/items/app%231/demo.zip");
    assert_eq!(missing.status, 404, "{missing:?}");
    assert_eq!(missing.header("content-type"), Some("application/json"));

    let png = file(&repo, "shot.png", PNG);
    let gif = file(&repo, "flow.gif", GIF);
    for path in [&png, &gif, &png] {
        env.mg(&["attach", "app#1", arg(path)]).assert_code(0);
    }
    let url = item(&env)["demo"]["zip_url"].as_str().unwrap().to_owned();
    assert_eq!(url, "/api/items/app%231/demo.zip");

    let zip = get_bytes(&env, &url);
    assert_eq!(zip.status, 200, "{:?}", zip.headers);
    assert_eq!(zip.header("content-type"), Some("application/zip"));
    assert_eq!(
        zip.header("content-disposition"),
        Some("attachment; filename=\"app-1-r1-demo.zip\"")
    );
    let length = zip.body.len().to_string();
    assert_eq!(zip.header("content-length"), Some(length.as_str()));
    let body = zip.body;
    assert!(body.starts_with(b"PK\x03\x04"));
    // Stored as they are, under names a folder holds side by side.
    assert!(contains(&body, PNG) && contains(&body, GIF));
    for name in ["shot.png", "flow.gif", "shot (2).png"] {
        assert!(contains(&body, name.as_bytes()), "{name}");
    }
}

#[test]
fn f16_a_closed_item_takes_no_attachments_and_drops_them_after_the_ttl() {
    let (env, repo) = hub_with_work();
    let gif = file(&repo, "flow.gif", GIF);
    let submitted = env
        .mg_at(repo.root(), &["--json", "submit", "--attach", arg(&gif)])
        .assert_code(0)
        .json();
    let url = submitted["attachments"][0]["url"]
        .as_str()
        .unwrap()
        .to_owned();

    let set = env
        .mg(&["--json", "hub", "set", "attachment-ttl", "0s"])
        .assert_code(0)
        .json();
    assert_eq!(set, json!({"attachment_ttl": "0s", "listen": "local"}));
    let text = env
        .mg(&["hub", "set", "attachment-ttl", "48h"])
        .assert_code(0)
        .stdout;
    assert_eq!(text.trim_end(), "attachment-ttl 2d");
    let invalid = env
        .mg(&["--json", "hub", "set", "attachment-ttl", "soon"])
        .json();
    assert_eq!(error_code(&invalid), "invalid_setting", "{invalid}");
    let unknown = env.mg(&["--json", "hub", "set", "colour", "red"]).json();
    assert_eq!(error_code(&unknown), "unknown_setting", "{unknown}");

    env.mg(&["--json", "abandon", "app#1"]).assert_code(0);
    let closed = env.mg(&["--json", "attach", "app#1", arg(&gif)]).json();
    assert_eq!(error_code(&closed), "item_closed", "{closed}");
    // Sending the same files again can't help, so no retry is hinted.
    assert_eq!(closed["error"]["hint"], Value::Null, "{closed}");
    let unknown = env.mg(&["--json", "attach", "nope#1", arg(&gif)]).json();
    assert_eq!(error_code(&unknown), "item_not_found", "{unknown}");
    assert_eq!(unknown["error"]["hint"], "mg inbox", "{unknown}");
    assert_eq!(get(&env, &url, "").status, 200, "kept for the 2d TTL");

    env.mg(&["--json", "hub", "set", "attachment-ttl", "0s"])
        .assert_code(0);
    let deadline = Instant::now() + Duration::from_secs(10);
    while get(&env, &url, "").status != 404 {
        assert!(Instant::now() < deadline, "the attachment was never pruned");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(item(&env)["demo"], Value::Null);
    let files = std::fs::read_dir(env.home().join("attachments")).unwrap();
    assert_eq!(files.count(), 0);
}
