//! Whether `mg skill install` wrote this `mg`'s skill to the home, and
//! whether the copies agents load are this `mg`'s.

use std::fs;
use std::path::{Path, PathBuf};

use api::doctor::{Check, CheckStatus};

/// Where Claude Code and Codex load the skill from, as the install prompt
/// links it. Other places one installed with `--dir` went are the agent's
/// to keep current.
pub(super) fn loaded() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    [".claude", ".codex"]
        .iter()
        .map(|agent| {
            Path::new(&home)
                .join(agent)
                .join("skills")
                .join(::skill::NAME)
        })
        .collect()
}

/// `files` as `skill::FILES` lists them; `installed` is the home's folder,
/// `loaded` where agents may load it from.
pub(super) fn check(installed: &Path, loaded: &[PathBuf], files: &[(&str, &str)]) -> Check {
    let current = |path: &Path| {
        files.iter().all(|(file, text)| {
            fs::read(path.join(file)).is_ok_and(|on_disk| on_disk == text.as_bytes())
        })
    };
    let check = |status, message: String, fix: Option<String>| Check {
        name: "skill".into(),
        status,
        message,
        fix,
    };
    let install = || Some("mg skill install".to_owned());
    // Agents read that copy, whatever the home holds.
    if let Some(stale) = loaded.iter().find(|path| path.exists() && !current(path)) {
        let fix = match current(installed) {
            true => Some(format!(
                "ln -sfn {} {}",
                installed.display(),
                stale.display()
            )),
            false => install(),
        };
        return check(
            CheckStatus::Warn,
            format!("{} is from another mg", stale.display()),
            fix,
        );
    }
    if current(installed) {
        return check(
            CheckStatus::Ok,
            format!("installed in {}", installed.display()),
            None,
        );
    }
    // A home other than the one the skill came from, as a dev hub's.
    if let Some(found) = loaded.iter().find(|path| current(path)) {
        return check(
            CheckStatus::Ok,
            format!("installed in {}", found.display()),
            None,
        );
    }
    match installed.exists() {
        true => check(
            CheckStatus::Warn,
            format!("{} is from another mg", installed.display()),
            install(),
        ),
        false => check(
            CheckStatus::Warn,
            format!("not installed in {}", installed.display()),
            install(),
        ),
    }
}

#[cfg(test)]
#[path = "skill_tests.rs"]
mod tests;
