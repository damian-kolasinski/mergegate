//! Which `git` executable to run.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// macOS's `git` on `PATH` when no other is installed: xcrun's shim, which
/// looks the developer tools up again on every run. That costs about 7 ms a
/// call against git's own 4 ms, and a submit runs git a dozen times.
const SHIM: &str = "/usr/bin/git";

/// Where xcrun reads the developer directory `xcode-select -s` chose.
const SELECTED: &str = "/var/db/xcode_select_link";

/// The `git` to run, resolved once per process: the one on `PATH`, or the
/// binary behind macOS's shim, found as xcrun would find it but without
/// running anything. Falls back to plain `git` whenever that can't be
/// settled, so the shim still answers.
pub fn binary() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    let git = BINARY.get_or_init(|| {
        let developer = std::env::var_os("DEVELOPER_DIR")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::fs::read_link(SELECTED).ok());
        resolve(
            std::env::var_os("PATH").as_deref(),
            developer.as_deref(),
            |path| path.is_file(),
        )
    });
    // The hub runs for days, and an Xcode upgrade removes the old Xcode and
    // its git with it. A stat is microseconds against the shim's 7 ms.
    if git.is_absolute() && !git.is_file() {
        return Path::new("git");
    }
    git
}

fn resolve(
    path: Option<&OsStr>,
    developer: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    let plain = PathBuf::from("git");
    if !cfg!(target_os = "macos") {
        return plain;
    }
    let first = path
        .into_iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join("git"))
        .find(|git| exists(git));
    if first.as_deref() != Some(Path::new(SHIM)) {
        return plain;
    }
    let developers = [
        developer.map(Path::to_path_buf),
        Some(PathBuf::from("/Applications/Xcode.app/Contents/Developer")),
        Some(PathBuf::from("/Library/Developer/CommandLineTools")),
    ];
    developers
        .into_iter()
        .flatten()
        // DEVELOPER_DIR may name Xcode.app itself, as xcrun accepts.
        .flat_map(|dir| {
            [
                dir.join("usr/bin/git"),
                dir.join("Contents/Developer/usr/bin/git"),
            ]
        })
        .find(|git| exists(git))
        .unwrap_or(plain)
}

#[cfg(test)]
#[path = "binary_tests.rs"]
mod tests;
