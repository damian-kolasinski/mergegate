use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use super::{Role, Spec, through_latest};

#[test]
fn a_unit_runs_this_mg_with_its_arguments() {
    let spec = Spec {
        role: Role::Hub,
        exe: PathBuf::from("/opt/mg"),
        args: vec!["--json".into(), "hub".into(), "start".into()],
        home: PathBuf::from("/h"),
        outcome: PathBuf::from("/h/hub.outcome"),
        log: PathBuf::from("/h/hub.log"),
        path_env: String::new(),
    };
    assert_eq!(spec.command(), ["/opt/mg", "--json", "hub", "start"]);
}

/// A mise install of `versions`, each with its `.mise-bins` link, and
/// `latest` pointing at `newest` as mise points it.
fn mise(root: &Path, versions: &[&str], newest: &str) -> PathBuf {
    let tool = root.join("installs/mergegate");
    for version in versions {
        fs::create_dir_all(tool.join(version).join(".mise-bins")).unwrap();
        fs::write(tool.join(version).join("mg"), version).unwrap();
        symlink("../mg", tool.join(version).join(".mise-bins/mg")).unwrap();
    }
    symlink(format!("./{newest}"), tool.join("latest")).unwrap();
    tool
}

#[test]
fn the_newest_mise_version_runs_through_its_latest_link() {
    let dir = tempfile::tempdir().unwrap();
    let tool = mise(dir.path(), &["0.6.0", "0.6.1"], "0.6.1");
    assert_eq!(
        through_latest(&tool.join("0.6.1/.mise-bins/mg")),
        tool.join("latest/.mise-bins/mg")
    );
    assert_eq!(
        through_latest(&tool.join("0.6.1/mg")),
        tool.join("latest/mg")
    );
    assert_eq!(
        through_latest(&tool.join("latest/mg")),
        tool.join("latest/mg")
    );
}

#[test]
fn a_version_latest_doesnt_point_at_keeps_its_own_path() {
    let dir = tempfile::tempdir().unwrap();
    let tool = mise(dir.path(), &["0.6.0", "0.6.1"], "0.6.1");
    let older = tool.join("0.6.0/.mise-bins/mg");
    assert_eq!(through_latest(&older), older);
}

#[test]
fn an_mg_without_a_latest_link_keeps_its_own_path() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("bin/mg");
    fs::create_dir_all(exe.parent().unwrap()).unwrap();
    fs::write(&exe, "mg").unwrap();
    assert_eq!(through_latest(&exe), exe);
}
