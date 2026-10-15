use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use api::doctor::CheckStatus;

use super::{first_on, judge};

fn executable(dir: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let file = dir.join(name);
    fs::write(&file, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    file
}

#[test]
fn the_first_executable_mg_on_path_is_the_one_found() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain");
    fs::create_dir_all(&plain).unwrap();
    fs::write(plain.join("mg"), "not executable").unwrap();
    let first = executable(&dir.path().join("first"), "mg");
    executable(&dir.path().join("second"), "mg");
    let path = std::env::join_paths([
        dir.path().join("missing"),
        plain,
        dir.path().join("first"),
        dir.path().join("second"),
    ])
    .unwrap();
    assert_eq!(first_on(&path), Some(first));
    assert_eq!(first_on(&std::env::join_paths([dir.path()]).unwrap()), None);
}

#[test]
fn this_mg_first_on_path_is_ok() {
    let dir = tempfile::tempdir().unwrap();
    let this = executable(&dir.path().join("bin"), "mg");
    let linked = dir.path().join("links");
    fs::create_dir_all(&linked).unwrap();
    symlink(&this, linked.join("mg")).unwrap();
    for found in [&this, &linked.join("mg")] {
        let check = judge(&this, Some(found), Path::new("/usr/bin/mg"));
        assert_eq!(check.status, CheckStatus::Ok, "{check:?}");
        assert_eq!(check.fix, None);
    }
}

#[test]
fn the_macos_editor_first_on_path_fails_with_the_folder_to_put_ahead() {
    let dir = tempfile::tempdir().unwrap();
    let this = executable(&dir.path().join("bin"), "mg");
    let editor = executable(&dir.path().join("usr/bin"), "mg");
    let check = judge(&this, Some(&editor), &editor);
    assert_eq!(check.status, CheckStatus::Fail);
    assert!(check.message.contains("macOS's text editor"), "{check:?}");
    assert_eq!(
        check.fix,
        Some(format!(
            "export PATH=\"{}:$PATH\"",
            dir.path().join("bin").display()
        ))
    );
}

#[test]
fn another_mg_or_none_on_path_warns() {
    let dir = tempfile::tempdir().unwrap();
    let this = executable(&dir.path().join("bin"), "mg");
    let other = executable(&dir.path().join("other"), "mg");
    let editor = Path::new("/usr/bin/mg");
    let check = judge(&this, Some(&other), editor);
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(check.message.starts_with("mg on PATH is "), "{check:?}");
    let check = judge(&this, None, editor);
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(check.message.starts_with("no mg on PATH"), "{check:?}");
    assert!(check.fix.is_some());
}

/// A mise data folder with `mise` and its `mg` shim, returning the shim.
fn mise_shim(data: &Path) -> PathBuf {
    let mise = executable(&data.join("bin"), "mise");
    let shims = data.join("shims");
    fs::create_dir_all(&shims).unwrap();
    symlink(&mise, shims.join("mg")).unwrap();
    shims.join("mg")
}

#[test]
fn a_mise_shim_is_ok_for_an_mg_mise_installed() {
    let dir = tempfile::tempdir().unwrap();
    let shim = mise_shim(dir.path());
    let this = executable(&dir.path().join("installs/mg/0.1.0"), "mg");
    let check = judge(&this, Some(&shim), Path::new("/usr/bin/mg"));
    assert_eq!(check.status, CheckStatus::Ok, "{check:?}");
}

#[test]
fn a_mise_shim_warns_for_an_mg_mise_did_not_install() {
    let dir = tempfile::tempdir().unwrap();
    let shim = mise_shim(&dir.path().join("mise"));
    let this = executable(&dir.path().join("cargo/bin"), "mg");
    let check = judge(&this, Some(&shim), Path::new("/usr/bin/mg"));
    assert_eq!(check.status, CheckStatus::Warn, "{check:?}");
}

#[test]
fn an_mg_mise_installed_puts_its_shims_ahead_not_its_versioned_folder() {
    let dir = tempfile::tempdir().unwrap();
    mise_shim(dir.path());
    let this = executable(&dir.path().join("installs/mg/0.1.0"), "mg");
    let editor = executable(&dir.path().join("usr/bin"), "mg");
    let check = judge(&this, Some(&editor), &editor);
    assert_eq!(check.status, CheckStatus::Fail);
    assert_eq!(
        check.fix,
        Some(format!(
            "export PATH=\"{}:$PATH\"",
            dir.path().join("shims").display()
        ))
    );
}
