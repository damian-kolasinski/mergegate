//! Whether `mg skill install` wrote this `mg`'s skill to the home.

use std::fs;
use std::path::Path;

use api::doctor::{Check, CheckStatus};

/// `files` as `skill::FILES` lists them. Only the default folder is looked
/// at: one installed with `--dir` is the agent's to keep current.
pub(super) fn check(path: &Path, files: &[(&str, &str)]) -> Check {
    let fix = Some("mg skill install".to_owned());
    if !path.exists() {
        return Check {
            name: "skill".into(),
            status: CheckStatus::Warn,
            message: format!("not installed in {}", path.display()),
            fix,
        };
    }
    let current = files.iter().all(|(file, text)| {
        fs::read(path.join(file)).is_ok_and(|on_disk| on_disk == text.as_bytes())
    });
    if !current {
        return Check {
            name: "skill".into(),
            status: CheckStatus::Warn,
            message: format!("{} is from another mg", path.display()),
            fix,
        };
    }
    Check {
        name: "skill".into(),
        status: CheckStatus::Ok,
        message: format!("installed in {}", path.display()),
        fix: None,
    }
}

#[cfg(test)]
#[path = "skill_tests.rs"]
mod tests;
