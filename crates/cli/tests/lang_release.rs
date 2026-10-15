//! Language packs released with mg: listed and installed by name, detected
//! from a project's files, and downloaded again once built for another mg.
//! The release is a local file server, and the hub reads its index from
//! `MG_LANGUAGES_INDEX` instead of the one baked into a release build.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use testkit::{Env, StaticFiles, TestRepo};

const FILE: &str = "json-0.24.8.tar.gz";

/// A pack built by `langs/build.mjs`, kept small enough to commit.
fn json_pack() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../highlight/tests/fixtures")
        .join(FILE)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A hub whose mg was released with `archive` as its json pack.
struct Release {
    env: Env,
    files: StaticFiles,
    _index: TempDir,
}

impl Release {
    fn of(archive: Vec<u8>) -> Release {
        let files = StaticFiles::start();
        let index = tempfile::tempdir().unwrap();
        let path = index.path().join("languages.json");
        let packs = json!({"packs": [{
            "name": "json",
            "version": "0.24.8",
            "file": FILE,
            "sha256": sha256(&archive),
        }]});
        std::fs::write(&path, packs.to_string()).unwrap();
        files.put(FILE, archive);
        let env = Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(vec![
            ("MG_LANGUAGES_INDEX".into(), path.display().to_string()),
            ("MG_LANGUAGES_URL".into(), files.url()),
        ]);
        env.start_hub();
        Release {
            env,
            files,
            _index: index,
        }
    }

    fn mg(&self, args: &[&str]) -> testkit::Run {
        self.env.mg(args)
    }
}

/// A repo whose main branch holds a json file.
fn repo() -> TestRepo {
    let repo = TestRepo::new("app");
    repo.commit_files("Config", &[("config.json", "{\"a\": 1}\n")]);
    repo
}

fn names(packs: &Value) -> Vec<&str> {
    packs["packs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect()
}

#[test]
fn lists_and_installs_released_packs_by_name() {
    let release = Release::of(std::fs::read(json_pack()).unwrap());
    let available = release
        .mg(&["--json", "lang", "list", "--available"])
        .assert_code(0)
        .json();
    assert_eq!(names(&available), ["json"]);
    assert_eq!(available["packs"][0]["installed"], Value::Null);

    let added = release
        .mg(&["--json", "lang", "add", "json"])
        .assert_code(0)
        .json();
    assert_eq!(names(&added), ["json"]);
    let text = release
        .mg(&["lang", "list", "--available"])
        .assert_code(0)
        .stdout;
    assert!(text.trim_end().ends_with("(installed)"), "{text}");

    let unknown = release
        .mg(&["--json", "lang", "add", "cobol"])
        .assert_code(1)
        .json();
    assert_eq!(unknown["error"]["code"], "pack_not_released");
    assert_eq!(unknown["error"]["hint"], "mg lang list --available");
}

#[test]
fn an_archive_not_matching_the_release_installs_nothing() {
    let release = Release::of(std::fs::read(json_pack()).unwrap());
    release.files.put(FILE, "tampered");
    let failed = release
        .mg(&["--json", "lang", "add", "json"])
        .assert_code(1)
        .json();
    assert_eq!(failed["error"]["code"], "download_failed");
    let listed = release
        .mg(&["--json", "lang", "list"])
        .assert_code(0)
        .json();
    assert_eq!(listed, json!({"packs": []}));
}

#[test]
fn adding_a_project_asks_for_the_packs_its_files_use() {
    let release = Release::of(std::fs::read(json_pack()).unwrap());
    let repo = repo();
    let asked = release
        .env
        .mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(2)
        .json();
    let decision = &asked["needs_decision"];
    assert_eq!(decision["flag"], "--languages");
    assert_eq!(decision["default"], "json");
    assert!(
        decision["question"]
            .as_str()
            .unwrap()
            .contains("json (1 file)"),
        "{decision}"
    );
    // Asking registered nothing.
    let projects = release.mg(&["--json", "project", "show"]).json();
    assert_eq!(projects["projects"], json!([]));

    let added = release
        .env
        .mg_at(
            repo.root(),
            &["--json", "project", "add", "--languages", "json"],
        )
        .assert_code(0)
        .json();
    assert_eq!(added["name"], "app");
    assert_eq!(added["languages"][0]["name"], "json");
    let listed = release
        .mg(&["--json", "lang", "list"])
        .assert_code(0)
        .json();
    assert_eq!(names(&listed), ["json"]);
}

#[test]
fn a_project_added_without_packs_gets_them_by_detection() {
    let release = Release::of(std::fs::read(json_pack()).unwrap());
    let repo = repo();
    let added = release
        .env
        .mg_at(
            repo.root(),
            &["--json", "project", "add", "--languages", "none"],
        )
        .assert_code(0)
        .json();
    assert_eq!(added["languages"], Value::Null);

    let doctor = release.mg(&["--json", "doctor"]).json();
    let check = checks(&doctor, "languages app");
    assert_eq!(check["status"], "warn");
    assert_eq!(check["fix"], "mg lang add --detect app");

    let detected = release
        .env
        .mg_at(repo.root(), &["--json", "lang", "add", "--detect"])
        .assert_code(0)
        .json();
    assert_eq!(names(&detected), ["json"]);
    let again = release
        .mg(&["lang", "add", "--detect", "app"])
        .assert_code(0)
        .stdout;
    assert_eq!(again.trim_end(), "every pack its files use is installed");
    let doctor = release.mg(&["--json", "doctor"]).json();
    assert_eq!(checks(&doctor, "languages app"), &Value::Null);
}

#[test]
fn packs_built_for_another_mg_are_downloaded_again() {
    let other = recompressed(&json_pack());
    let release = Release::of(other);
    let pack = json_pack();
    release
        .mg(&["--json", "lang", "add", "--from", pack.to_str().unwrap()])
        .assert_code(0);

    let doctor = release.mg(&["--json", "doctor"]).json();
    let check = checks(&doctor, "languages");
    assert_eq!(check["status"], "warn");
    assert_eq!(check["fix"], "mg lang update");
    let available = release
        .mg(&["--json", "lang", "list", "--available"])
        .json();
    assert_eq!(available["packs"][0]["outdated"], true);

    let updated = release
        .mg(&["--json", "lang", "update"])
        .assert_code(0)
        .json();
    assert_eq!(names(&updated), ["json"]);
    let again = release.mg(&["lang", "update"]).assert_code(0).stdout;
    assert_eq!(again.trim_end(), "every pack is up to date");
    let doctor = release.mg(&["--json", "doctor"]).json();
    assert_eq!(checks(&doctor, "languages")["status"], "ok");
}

#[test]
fn a_build_without_its_release_installs_only_from_files() {
    let env = Env::new(env!("CARGO_BIN_EXE_mg"));
    env.start_hub();
    let available = env
        .mg(&["--json", "lang", "list", "--available"])
        .assert_code(0)
        .json();
    assert_eq!(available, json!({"packs": []}));
    let failed = env
        .mg(&["--json", "lang", "add", "json"])
        .assert_code(1)
        .json();
    assert_eq!(failed["error"]["code"], "no_releases");
    // Nothing to ask about: no pack can be downloaded.
    let repo = repo();
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
}

fn checks<'a>(doctor: &'a Value, name: &str) -> &'a Value {
    doctor["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or(&Value::Null)
}

/// The same pack in another archive: the tarball gzipped again.
fn recompressed(archive: &Path) -> Vec<u8> {
    let tar = Command::new("gzip")
        .arg("-dc")
        .arg(archive)
        .output()
        .unwrap();
    assert!(tar.status.success());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pack.tar");
    std::fs::write(&path, tar.stdout).unwrap();
    let gz = Command::new("gzip")
        .args(["-1", "-n", "-c"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(gz.status.success());
    assert_ne!(gz.stdout, std::fs::read(archive).unwrap());
    gz.stdout
}
