use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use testkit::{Env, FakeGithub, TestRepo};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

fn read_zip(path: &Path, entry: &str) -> String {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut text = String::new();
    zip.by_name(entry)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

#[test]
fn running_hub_records_requests_without_payloads_and_collects_after_shutdown() {
    let env = env();
    env.start_hub();
    // The request has attacker-controlled text; none belongs in telemetry.
    let addr = env.hub_addr();
    let response = testkit::http_reply(
        &addr,
        &format!(
            "GET /secret-ghp_shouldneverappear?token=hunter2 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        ),
    );
    assert_eq!(response.status, 404);
    assert!(
        response
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("x-mergegate-request-id"))
    );
    let failure = env
        .mg(&["--json", "feedback", "missing#123"])
        .assert_code(1)
        .json();
    let request_id = failure["error"]["request_id"]
        .as_str()
        .expect("correlated failure");
    env.mg(&["hub", "stop"]).assert_code(0);
    let out = env.home().join("bundle.zip");
    let value = env
        .mg(&[
            "--json",
            "diagnostics",
            "collect",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert_code(0)
        .json();
    assert_eq!(value["archive"], out.to_str().unwrap());
    let snapshot = read_zip(&out, "diagnostics.json");
    assert!(snapshot.contains("request_finished"), "{snapshot}");
    assert!(snapshot.contains(request_id), "{snapshot}");
    assert!(snapshot.contains("unmatched"), "{snapshot}");
    assert!(!snapshot.contains("shouldneverappear"));
    assert!(!snapshot.contains("hunter2"));
    assert!(!snapshot.contains(env.home().to_str().unwrap()));
    let parsed: Value = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(parsed["format_version"], 1);
    assert!(
        parsed["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "hub_unreachable_or_incompatible_local_files_only")
    );
    env.mg(&[
        "--json",
        "diagnostics",
        "collect",
        "--out",
        out.to_str().unwrap(),
    ])
    .assert_code(1);
    assert_eq!(snapshot, read_zip(&out, "diagnostics.json"));
}

#[test]
fn collection_does_not_need_a_database_and_bounds_rotation() {
    let env = env().with_vars(vec![("MG_TELEMETRY_FILE_BYTES".into(), "1024".into())]);
    env.start_hub();
    for _ in 0..30 {
        env.mg(&["hub", "status"]).assert_code(0);
    }
    env.mg(&["hub", "stop"]).assert_code(0);
    let files = fs::read_dir(env.home().join("telemetry"))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert!(files.len() <= 5);
    assert!(files.iter().all(|f| f.metadata().unwrap().len() <= 1024));
    fs::write(env.home().join("hub.db"), b"broken database").unwrap();
    let out = env.home().join("broken.zip");
    env.mg(&[
        "--json",
        "diagnostics",
        "collect",
        "--out",
        out.to_str().unwrap(),
    ])
    .assert_code(0);
    let snapshot: Value = serde_json::from_str(&read_zip(&out, "diagnostics.json")).unwrap();
    assert!(!snapshot["records"].as_array().unwrap().is_empty());
}

fn prepared(env: &Env) -> std::path::PathBuf {
    let body = env.home().join("summary.txt");
    let logs = env.home().join("selected.txt");
    fs::write(&body, "The hub returned an error. Expected: submit succeeds. Actual: 500.\nAuthorization: Bearer summarysecret\n").unwrap();
    fs::write(&logs, "request_failed error_code=store_failed\nGH_TOKEN=ghp_shouldnotleave\n/Users/alice/private\n").unwrap();
    let out = env.home().join("report");
    env.mg(&[
        "--json",
        "report",
        "prepare",
        "--title",
        "Submit fails",
        "--body",
        body.to_str().unwrap(),
        "--logs",
        logs.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ])
    .assert_code(0);
    out
}

#[test]
fn prepare_send_and_retry_keep_the_archive_local_and_create_one_issue() {
    let github = FakeGithub::start();
    let repo = TestRepo::new("mergegate");
    github.create_repo("damian-kolasinski/mergegate", repo.root());
    github.deny_push("damian-kolasinski/mergegate"); // A reporter needs no repository write access.
    let env = env().with_vars(github.env());
    env.start_hub();
    let out = prepared(&env);
    assert!(github.issues().is_empty());
    let draft = fs::read_to_string(out.join("report.md")).unwrap();
    let archive = read_zip(&out.join("diagnostics.zip"), "report.md");
    assert_eq!(archive, draft);
    for secret in ["summarysecret", "shouldnotleave", "alice"] {
        assert!(!draft.contains(secret), "{draft}");
    }
    assert!(draft.contains("error_code=store_failed"));
    // An edit after preparation is sanitized again at the publication boundary.
    fs::write(
        out.join("report.md"),
        format!("{draft}\nAuthorization: Bearer editedsecret\n"),
    )
    .unwrap();
    let args = ["--json", "report", "send", "--from", out.to_str().unwrap()];
    let first = env.mg(&args).assert_code(0).json();
    assert_eq!(first["issue"]["number"], 1);
    assert_eq!(github.issues().len(), 1);
    let issue = &github.issues()[0];
    assert_eq!(issue.repo, "damian-kolasinski/mergegate");
    assert!(!issue.body.contains("editedsecret"));
    assert!(!read_zip(&out.join("diagnostics.zip"), "report.md").contains("editedsecret"));
    assert!(!issue.body.contains("diagnostics.json"));
    assert_eq!(env.mg(&args).assert_code(0).json(), first);
    // Simulate losing the receipt after GitHub accepted the POST.
    fs::remove_file(out.join("issue.json")).unwrap();
    fs::write(out.join("send.pending"), "uncertain").unwrap();
    let error = env.mg(&args).assert_code(1).json();
    assert_eq!(error["error"]["code"], "report_uncertain");
    let mut retry = args.to_vec();
    retry.push("--retry");
    github.hide_issues(true);
    let lagging = env.mg(&retry).assert_code(1).json();
    assert_eq!(lagging["error"]["code"], "report_uncertain");
    assert_eq!(
        github.issues().len(),
        1,
        "a stale issue list must not cause another POST"
    );
    github.hide_issues(false);
    assert_eq!(env.mg(&retry).assert_code(0).json(), first);
    assert_eq!(github.issues().len(), 1);
    assert!(out.join("diagnostics.zip").exists());
}

#[test]
fn failed_authentication_preserves_the_draft_and_zip_for_retry() {
    let github = FakeGithub::start();
    let repo = TestRepo::new("mergegate");
    github.create_repo("damian-kolasinski/mergegate", repo.root());
    let env = env().with_vars(github.env());
    let out = prepared(&env);
    github.log_out();
    let args = ["--json", "report", "send", "--from", out.to_str().unwrap()];
    let error = env.mg(&args).assert_code(1).json();
    assert_eq!(error["error"]["code"], "report_send_failed");
    assert!(out.join("report.md").exists());
    assert!(out.join("diagnostics.zip").exists());
    assert!(github.issues().is_empty());
    github.log_in("gho_retry");
    let mut retry = args.to_vec();
    retry.push("--retry");
    env.mg(&retry).assert_code(0);
    assert_eq!(github.issues().len(), 1);
}

#[test]
fn logs_report_the_running_hub_version() {
    let env = env();
    env.start_hub();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = env
            .mg(&["--json", "diagnostics", "logs", "--since", "1h"])
            .assert_code(0)
            .json();
        assert!(snapshot["hub_version"].is_string());
        if snapshot["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["fields"]["event_code"] == "hub_starting")
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let invalid = env
        .mg(&["--json", "diagnostics", "logs", "--since", "999d"])
        .assert_code(64)
        .json();
    assert_eq!(invalid["error"]["code"], json!("usage"));
}

#[test]
fn warning_details_are_opt_in_and_never_enter_default_archives() {
    let env = env().with_vars(vec![(
        "MG_LANGUAGES_INDEX".into(),
        "/Users/private/missing-index".into(),
    )]);
    env.start_hub();
    env.mg(&["hub", "stop"]).assert_code(0);
    let detailed = env
        .mg(&["--json", "diagnostics", "logs", "--details"])
        .assert_code(0)
        .json();
    let details = detailed["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["detail"].as_str())
        .collect::<Vec<_>>();
    assert!(
        details
            .iter()
            .any(|d| d.contains("can't read MG_LANGUAGES_INDEX")),
        "{detailed}"
    );
    assert!(!detailed.to_string().contains("/Users/private"));
    let plain = env
        .mg(&["--json", "diagnostics", "logs"])
        .assert_code(0)
        .json();
    assert!(
        plain["records"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r.get("detail").is_none())
    );
    assert_eq!(plain["hub_version"], detailed["hub_version"]);
    let version = env.mg(&["--version"]).assert_code(0).stdout;
    assert_eq!(
        plain["hub_version"],
        version.trim().strip_prefix("mg ").unwrap()
    );
    let out = env.home().join("warnings.zip");
    env.mg(&["diagnostics", "collect", "--out", out.to_str().unwrap()])
        .assert_code(0);
    assert!(!read_zip(&out, "diagnostics.json").contains("can't read MG_LANGUAGES_INDEX"));
}

#[test]
fn post_stage_auth_rejection_allows_sending_the_same_draft_after_login() {
    let github = FakeGithub::start();
    let repo = TestRepo::new("mergegate");
    github.create_repo("damian-kolasinski/mergegate", repo.root());
    let env = env().with_vars(github.env());
    let out = prepared(&env);
    github.reject_issue_auth(true);
    let args = ["--json", "report", "send", "--from", out.to_str().unwrap()];
    let failed = env.mg(&args).assert_code(1).json();
    assert_eq!(failed["error"]["code"], "report_send_failed");
    assert!(
        !out.join("send.pending").exists(),
        "a rejected POST is safe to retry"
    );
    assert!(github.issues().is_empty());
    github.reject_issue_auth(false);
    let mut retry = args.to_vec();
    retry.push("--retry");
    env.mg(&retry).assert_code(0);
    assert_eq!(github.issues().len(), 1);
}
