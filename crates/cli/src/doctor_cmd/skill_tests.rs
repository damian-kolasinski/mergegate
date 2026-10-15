use std::fs;
use std::path::{Path, PathBuf};

use api::doctor::{Check, CheckStatus};

use super::{check, same};

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
    let check = check(&dir.path().join("mergegate"), true, &[], FILES);
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(check.message.starts_with("not installed in "), "{check:?}");
    assert_eq!(check.fix.as_deref(), Some("mg skill install"));
}

#[test]
fn the_skill_this_mg_embeds_is_ok() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), FILES);
    assert_eq!(check(dir.path(), true, &[], FILES).status, CheckStatus::Ok);
}

#[test]
fn a_changed_or_missing_file_is_another_mgs_skill() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        &[("SKILL.md", "skill"), ("references/setup.md", "older")],
    );
    let changed = check(dir.path(), true, &[], FILES);
    assert_eq!(changed.status, CheckStatus::Warn);
    assert!(
        changed.message.ends_with(" is from another mg"),
        "{changed:?}"
    );
    assert_eq!(changed.fix.as_deref(), Some("mg skill install"));

    fs::remove_file(dir.path().join("references/setup.md")).unwrap();
    assert_eq!(
        check(dir.path(), true, &[], FILES).status,
        CheckStatus::Warn
    );
}

#[test]
fn the_copies_agents_load_count() {
    let dir = tempfile::tempdir().unwrap();
    let installed = dir.path().join("home/mergegate");
    let claude = dir.path().join("claude/mergegate");
    let codex = dir.path().join("codex/mergegate");
    let loaded: [PathBuf; 2] = [claude.clone(), codex.clone()];

    // A copy left behind by an upgrade is what that agent reads.
    write(&installed, FILES);
    write(&claude, FILES);
    write(&codex, &[("SKILL.md", "older")]);
    let stale = check(&installed, true, &loaded, FILES);
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

    // Linking to a home that is behind too wouldn't help.
    write(&installed, &[("SKILL.md", "older")]);
    let both = check(&installed, true, &loaded, FILES);
    assert_eq!(both.status, CheckStatus::Warn);
    assert_eq!(both.fix.as_deref(), Some("mg skill install"));
}

/// A home other than the default one, as a dev or test hub's, next to the
/// default home's skill, and the folder an agent loads the skill from.
struct Homes {
    _dir: tempfile::TempDir,
    other: PathBuf,
    default: PathBuf,
    loaded: PathBuf,
}

impl Homes {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let loaded = dir.path().join("claude/skills/mergegate");
        fs::create_dir_all(loaded.parent().unwrap()).unwrap();
        Self {
            other: dir.path().join("dev/skill/mergegate"),
            default: dir.path().join("default/skill/mergegate"),
            loaded,
            _dir: dir,
        }
    }

    fn link(&self, to: &Path) {
        std::os::unix::fs::symlink(to, &self.loaded).unwrap();
    }

    /// As `mg doctor` run with the other home sees it.
    fn checked_from_other(&self) -> Check {
        check(
            &self.other,
            false,
            std::slice::from_ref(&self.loaded),
            FILES,
        )
    }
}

#[test]
fn another_home_leaves_a_copy_linked_to_the_default_one_alone() {
    let homes = Homes::new();
    write(&homes.other, FILES);
    write(&homes.default, &[("SKILL.md", "older")]);
    homes.link(&homes.default);
    let check = homes.checked_from_other();
    assert_eq!(check.status, CheckStatus::Ok, "{check:?}");
    assert_eq!(
        check.message,
        format!("installed in {}", homes.other.display())
    );
    assert_eq!(check.fix, None);
}

#[test]
fn another_home_leaves_a_plain_copy_alone() {
    let homes = Homes::new();
    write(&homes.other, FILES);
    write(&homes.loaded, &[("SKILL.md", "older")]);
    let check = homes.checked_from_other();
    assert_eq!(check.status, CheckStatus::Ok, "{check:?}");
    assert_eq!(check.fix, None);
}

#[test]
fn another_home_is_satisfied_by_a_current_copy_agents_load() {
    let homes = Homes::new();
    write(&homes.default, FILES);
    homes.link(&homes.default);
    let check = homes.checked_from_other();
    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(
        check.message,
        format!("installed in {}", homes.loaded.display())
    );
}

#[test]
fn another_home_still_fixes_a_copy_linked_to_it() {
    let homes = Homes::new();
    write(&homes.other, &[("SKILL.md", "older")]);
    homes.link(&homes.other);
    let check = homes.checked_from_other();
    assert_eq!(check.status, CheckStatus::Warn);
    assert_eq!(
        check.message,
        format!("{} is from another mg", homes.loaded.display())
    );
    assert_eq!(check.fix.as_deref(), Some("mg skill install"));
}

#[test]
fn another_home_without_the_skill_says_to_install_it() {
    let homes = Homes::new();
    write(&homes.default, &[("SKILL.md", "older")]);
    homes.link(&homes.default);
    let check = homes.checked_from_other();
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(check.message.starts_with("not installed in "), "{check:?}");
    assert_eq!(check.fix.as_deref(), Some("mg skill install"));
}

#[test]
fn the_default_home_takes_back_a_copy_linked_to_another() {
    let homes = Homes::new();
    write(&homes.default, FILES);
    write(&homes.other, &[("SKILL.md", "older")]);
    homes.link(&homes.other);
    let check = check(
        &homes.default,
        true,
        std::slice::from_ref(&homes.loaded),
        FILES,
    );
    assert_eq!(check.status, CheckStatus::Warn);
    assert_eq!(
        check.fix,
        Some(format!(
            "ln -sfn {} {}",
            homes.default.display(),
            homes.loaded.display()
        ))
    );
}

#[test]
fn a_link_is_the_folder_it_points_at() {
    let homes = Homes::new();
    write(&homes.default, FILES);
    homes.link(&homes.default);
    assert!(same(&homes.loaded, &homes.default));
    assert!(!same(&homes.loaded, &homes.other));
    // Neither exists, as a home before its first install.
    assert!(same(&homes.other, &homes.other));
}
