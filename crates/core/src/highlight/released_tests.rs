use super::*;
use crate::fake::{FakeHighlighter, FakePackSource, digest, installed, pack};

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).into()).collect()
}

#[test]
fn available_packs_are_the_known_ones_released_at_their_version() {
    let known = [pack("json", "1"), pack("rust", "2"), pack("toml", "3")];
    let source = FakePackSource::with(&[("json", "1"), ("rust", "9")]);
    assert_eq!(available(&known, source.releases()), [pack("json", "1")]);
}

#[test]
fn resolving_finds_each_release_once() {
    let source = FakePackSource::with(&[("json", "1"), ("rust", "2")]);
    let found = resolve(source.releases(), &names(&["rust", "json", "rust"])).unwrap();
    let found: Vec<_> = found.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(found, ["rust", "json"]);
}

#[test]
fn resolving_fails_on_a_pack_not_released() {
    let source = FakePackSource::with(&[("json", "1")]);
    let err = resolve(source.releases(), &names(&["json", "cobol"])).unwrap_err();
    assert!(matches!(err, LangError::NotReleased(name) if name == "cobol"));
}

#[test]
fn without_releases_nothing_resolves() {
    let err = resolve(&[], &names(&["json"])).unwrap_err();
    assert!(matches!(err, LangError::NoReleases));
}

#[test]
fn packs_installed_from_another_archive_are_outdated() {
    let mut installed = installed(&[("json", "1"), ("rust", "2"), ("own", "1")]);
    installed
        .digests
        .insert("rust".into(), "built for another mg".into());
    let source = FakePackSource::with(&[("json", "1"), ("rust", "2")]);
    assert_eq!(outdated(&installed, source.releases()), ["rust"]);
}

#[test]
fn a_pack_released_at_another_version_is_outdated() {
    let installed = installed(&[("json", "1")]);
    let source = FakePackSource::with(&[("json", "2")]);
    assert_ne!(digest("json", "1"), digest("json", "2"));
    assert_eq!(outdated(&installed, source.releases()), ["json"]);
}

#[test]
fn install_downloads_every_pack_before_installing() {
    let source = FakePackSource::with(&[("json", "1"), ("rust", "2")]);
    let into = Path::new("/downloads");
    // The fake installs nothing: what matters is what reached it.
    let err = install(
        &source,
        &FakeHighlighter::default(),
        &names(&["rust", "json"]),
        into,
    );
    assert!(matches!(err, Err(LangError::Empty(path)) if path == into));
    assert_eq!(*source.downloaded.borrow(), ["rust", "json"]);
}

#[test]
fn install_downloads_nothing_when_a_name_is_unknown() {
    let source = FakePackSource::with(&[("json", "1")]);
    let err = install(
        &source,
        &FakeHighlighter::default(),
        &names(&["json", "cobol"]),
        Path::new("/downloads"),
    );
    assert!(matches!(err, Err(LangError::NotReleased(_))));
    assert!(source.downloaded.borrow().is_empty());
}

#[test]
fn a_failed_download_installs_nothing() {
    let mut source = FakePackSource::with(&[("json", "1")]);
    source.fail = Some("json".into());
    let err = install(
        &source,
        &FakeHighlighter::default(),
        &names(&["json"]),
        Path::new("/downloads"),
    );
    assert!(matches!(err, Err(LangError::Download { .. })));
}

#[test]
fn installing_no_names_does_nothing() {
    let source = FakePackSource::with(&[("json", "1")]);
    let installed = install(
        &source,
        &FakeHighlighter::default(),
        &[],
        Path::new("/downloads"),
    );
    assert_eq!(installed.unwrap(), []);
}
