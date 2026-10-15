use std::fs;
use std::path::{Path, PathBuf};

use api::doctor::CheckStatus;

use super::check;

const FILES: &[(&str, &str)] = &[("SKILL.md", "skill"), ("references/setup.md", "setup")];

fn write(dir: &Path, files: &[(&str, &str)]) {
    for (file, text) in files {
        let path = dir.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

#[test]
fn a_missing_skill_warns_with_the_install_command() {
    let dir = tempfile::tempdir().unwrap();
    let check = check(&dir.path().join("mergegate"), &[], FILES);
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(check.message.starts_with("not installed in "), "{check:?}");
    assert_eq!(check.fix.as_deref(), Some("mg skill install"));
}

#[test]
fn the_skill_this_mg_embeds_is_ok() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), FILES);
    assert_eq!(check(dir.path(), &[], FILES).status, CheckStatus::Ok);
}

#[test]
fn a_changed_or_missing_file_is_another_mgs_skill() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        &[("SKILL.md", "skill"), ("references/setup.md", "older")],
    );
    let changed = check(dir.path(), &[], FILES);
    assert_eq!(changed.status, CheckStatus::Warn);
    assert!(
        changed.message.ends_with(" is from another mg"),
        "{changed:?}"
    );
    assert_eq!(changed.fix.as_deref(), Some("mg skill install"));

    fs::remove_file(dir.path().join("references/setup.md")).unwrap();
    assert_eq!(check(dir.path(), &[], FILES).status, CheckStatus::Warn);
}

#[test]
fn the_copies_agents_load_count() {
    let dir = tempfile::tempdir().unwrap();
    let installed = dir.path().join("home/mergegate");
    let claude = dir.path().join("claude/mergegate");
    let codex = dir.path().join("codex/mergegate");
    let loaded: [PathBuf; 2] = [claude.clone(), codex.clone()];

    // Installed from another home, as a dev hub's doctor sees it.
    write(&claude, FILES);
    let found = check(&installed, &loaded, FILES);
    assert_eq!(found.status, CheckStatus::Ok);
    assert_eq!(found.message, format!("installed in {}", claude.display()));

    // A copy left behind by an upgrade is what that agent reads.
    write(&installed, FILES);
    write(&codex, &[("SKILL.md", "older")]);
    let stale = check(&installed, &loaded, FILES);
    assert_eq!(stale.status, CheckStatus::Warn);
    assert_eq!(
        stale.message,
        format!("{} is from another mg", codex.display())
    );
    assert_eq!(
        stale.fix,
        Some(format!(
            "ln -sfn {} {}",
            installed.display(),
            codex.display()
        ))
    );
}
