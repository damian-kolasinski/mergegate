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
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let zdotdir = std::env::var_os("ZDOTDIR").map(PathBuf::from);
    let profile = profile(
        std::env::var_os("SHELL").as_deref(),
        &home,
        zdotdir.as_deref(),
    );
    Some(judge(
        &this,
        first_on(&path).as_deref(),
        Path::new(MACOS_EDITOR),
        &shown(&profile, &home),
    ))
}

/// The file a login shell of `shell` reads, where a PATH set stays set:
/// macOS's `/etc/zprofile` runs `path_helper`, which puts `/usr/bin` back
/// ahead of what `~/.zshenv` set, and agents' login shells don't read
/// `~/.zshrc`.
fn profile(shell: Option<&OsStr>, home: &Path, zdotdir: Option<&Path>) -> PathBuf {
    let name = shell
        .map(Path::new)
        .and_then(Path::file_name)
        .and_then(OsStr::to_str);
    match name {
        Some("zsh") => zdotdir.unwrap_or(home).join(".zprofile"),
        // bash reads the first of these it finds, so a new
        // `~/.bash_profile` would hide an existing `~/.profile`.
        Some("bash") => [".bash_profile", ".bash_login"]
            .map(|file| home.join(file))
            .into_iter()
            .find(|file| file.exists())
            .unwrap_or_else(|| home.join(".profile")),
        _ => home.join(".profile"),
    }
}

/// `file` with `home` as `~`, as the operator would type it, quoted when
/// the shell would split or expand it.
fn shown(file: &Path, home: &Path) -> String {
    let quoted = |path: &Path| {
        let path = path.display().to_string();
        let plain = path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c));
        if plain {
            path
        } else {
            format!("'{}'", path.replace('\'', r"'\''"))
        }
    };
    match file.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("~/{}", quoted(rest)),
        _ => quoted(file),
    }
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

fn judge(this: &Path, found: Option<&Path>, editor: &Path, profile: &str) -> Check {
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
            _ => Some(format!(
                "echo 'export PATH=\"{}:$PATH\"' >> {profile}",
                ahead.display()
            )),
        },
    };
    let this_shown = this.display();
    let Some(found) = found else {
        return check(
            CheckStatus::Warn,
            format!("no mg on PATH, so agents can't run this one ({this_shown})"),
        );
    };
    // A release ships `mergegate` as a copy of the `mg` beside it.
    if same_file(found, this) || same_file(found, &this.with_file_name("mg")) {
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
                "mg on PATH is {}, a text editor (Micro GNU Emacs), not this mg ({this_shown})",
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
