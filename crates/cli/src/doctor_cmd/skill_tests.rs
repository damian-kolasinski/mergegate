use std::fs;

use api::doctor::CheckStatus;

use super::check;

const FILES: &[(&str, &str)] = &[("SKILL.md", "skill"), ("references/setup.md", "setup")];

fn write(dir: &std::path::Path, files: &[(&str, &str)]) {
    for (file, text) in files {
        let path = dir.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

#[test]
fn a_missing_skill_warns_with_the_install_command() {
    let dir = tempfile::tempdir().unwrap();
    let check = check(&dir.path().join("mergegate"), FILES);
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(check.message.starts_with("not installed in "), "{check:?}");
    assert_eq!(check.fix.as_deref(), Some("mg skill install"));
}

#[test]
fn the_skill_this_mg_embeds_is_ok() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), FILES);
    assert_eq!(check(dir.path(), FILES).status, CheckStatus::Ok);
}

#[test]
fn a_changed_or_missing_file_is_another_mgs_skill() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        &[("SKILL.md", "skill"), ("references/setup.md", "older")],
    );
    let changed = check(dir.path(), FILES);
    assert_eq!(changed.status, CheckStatus::Warn);
    assert!(
        changed.message.ends_with(" is from another mg"),
        "{changed:?}"
    );
    assert_eq!(changed.fix.as_deref(), Some("mg skill install"));

    fs::remove_file(dir.path().join("references/setup.md")).unwrap();
    assert_eq!(check(dir.path(), FILES).status, CheckStatus::Warn);
}
