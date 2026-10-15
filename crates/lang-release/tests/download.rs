use std::fs;

use mergegate_core::highlight::{LangError, PackSource};
use mergegate_lang_release::Releases;
use sha2::{Digest, Sha256};
use testkit::StaticFiles;

const FILE: &str = "json-0.24.8.tar.gz";

fn archive() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../highlight/tests/fixtures/json-0.24.8.tar.gz"
    );
    fs::read(path).unwrap()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn index(sha256: &str) -> String {
    format!(
        r#"{{"packs":[{{"name":"json","version":"0.24.8","file":"{FILE}","sha256":"{sha256}"}}]}}"#
    )
}

/// A release holding the json pack, and an mg whose index lists it at `sha`.
fn setup(sha: &str) -> (StaticFiles, Releases) {
    let files = StaticFiles::start();
    files.put(FILE, archive());
    let releases = Releases::new(&index(sha), format!("{}/", files.url())).unwrap();
    (files, releases)
}

#[test]
fn downloads_a_released_pack_into_the_directory() {
    let (_files, releases) = setup(&sha256(&archive()));
    assert_eq!(releases.releases().len(), 1);
    let into = tempfile::tempdir().unwrap();
    releases
        .download(&releases.releases()[0], into.path())
        .unwrap();
    assert_eq!(fs::read(into.path().join(FILE)).unwrap(), archive());
}

#[test]
fn an_archive_not_matching_its_digest_is_refused() {
    let (_files, releases) = setup(&"0".repeat(64));
    let into = tempfile::tempdir().unwrap();
    let err = releases
        .download(&releases.releases()[0], into.path())
        .unwrap_err();
    assert!(
        matches!(&err, LangError::Download { name, reason }
            if name == "json" && reason.contains("not the 0000")),
        "{err}"
    );
    assert!(fs::read_dir(into.path()).unwrap().next().is_none());
}

#[test]
fn a_missing_archive_says_what_the_release_answered() {
    let files = StaticFiles::start();
    let releases = Releases::new(&index(&sha256(&archive())), files.url()).unwrap();
    let into = tempfile::tempdir().unwrap();
    let err = releases
        .download(&releases.releases()[0], into.path())
        .unwrap_err();
    assert!(err.to_string().contains("answered 404"), "{err}");
}
