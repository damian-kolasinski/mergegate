//! Whether the `mg` on PATH is this one. Agents run `mg` by name, and macOS
//! ships an unrelated editor as `/usr/bin/mg` that a PATH listing `/usr/bin`
//! early finds first.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use api::doctor::{Check, CheckStatus};

use super::service::same_file;

const MACOS_EDITOR: &str = "/usr/bin/mg";

pub(super) fn check() -> Option<Check> {
    // Canonical, so a `mergegate` linked to this file is this `mg`.
    let this = fs::canonicalize(std::env::current_exe().ok()?).ok()?;
    let path = std::env::var_os("PATH").unwrap_or_default();
    Some(judge(
        &this,
        first_on(&path).as_deref(),
        Path::new(MACOS_EDITOR),
    ))
}

/// The `mg` a shell runs: the first executable file of that name in `path`.
fn first_on(path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join("mg"))
        .find(|mg| {
            fs::metadata(mg).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

fn judge(this: &Path, found: Option<&Path>, editor: &Path) -> Check {
    let shims = mise_shims(this);
    let ahead = shims
        .clone()
        .unwrap_or_else(|| this.parent().unwrap_or(this).to_owned());
    let check = |status, message: String| Check {
        name: "path".into(),
        status,
        message,
        fix: match status {
            CheckStatus::Ok => None,
            _ => Some(format!("export PATH=\"{}:$PATH\"", ahead.display())),
        },
    };
    let this_shown = this.display();
    let Some(found) = found else {
        return check(
            CheckStatus::Warn,
            format!("no mg on PATH, so agents can't run this one ({this_shown})"),
        );
    };
    if same_file(found, this) {
        return check(
            CheckStatus::Ok,
            format!("mg on PATH is this one, {this_shown}"),
        );
    }
    if shims.is_some() && is_mise_shim(found) {
        return check(
            CheckStatus::Ok,
            format!("mg on PATH is mise's shim, {}", found.display()),
        );
    }
    if same_file(found, editor) {
        return check(
            CheckStatus::Fail,
            format!(
                "mg on PATH is {}, macOS's text editor, not this mg ({this_shown})",
                found.display()
            ),
        );
    }
    check(
        CheckStatus::Warn,
        format!(
            "mg on PATH is {}, not this mg ({this_shown})",
            found.display()
        ),
    )
}

/// The shims folder of the mise that installed `this`, when one did. Its
/// installs are versioned folders, which `mise prune` removes, so the shims
/// are what belongs on PATH.
fn mise_shims(this: &Path) -> Option<PathBuf> {
    let installs = this
        .ancestors()
        .find(|dir| dir.file_name() == Some(OsStr::new("installs")))?;
    let shims = installs.parent()?.join("shims");
    shims.is_dir().then_some(shims)
}

/// mise's shims are links to mise, which runs the `mg` mise installed.
fn is_mise_shim(found: &Path) -> bool {
    fs::canonicalize(found).is_ok_and(|target| target.file_name() == Some(OsStr::new("mise")))
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
