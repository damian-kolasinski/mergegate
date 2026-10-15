use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::resolve;

/// `resolve` over a filesystem holding only `files`.
fn resolved(path: &str, developer: Option<&str>, files: &[&str]) -> PathBuf {
    resolve(Some(OsStr::new(path)), developer.map(Path::new), |file| {
        files.iter().any(|f| Path::new(f) == file)
    })
}

#[test]
#[cfg(not(target_os = "macos"))]
fn elsewhere_path_decides() {
    let git = resolved("/usr/bin", None, &["/usr/bin/git"]);
    assert_eq!(git, Path::new("git"));
}

#[test]
#[cfg(target_os = "macos")]
fn another_git_first_on_path_is_left_to_path() {
    let files = ["/opt/homebrew/bin/git", "/usr/bin/git", "/dev/usr/bin/git"];
    let git = resolved("/opt/homebrew/bin:/usr/bin", Some("/dev"), &files);
    assert_eq!(git, Path::new("git"));
}

#[test]
#[cfg(target_os = "macos")]
fn the_shim_gives_way_to_the_selected_developer_dir() {
    let files = [
        "/usr/bin/git",
        "/Applications/Xcode-26.app/Contents/Developer/usr/bin/git",
        "/Library/Developer/CommandLineTools/usr/bin/git",
    ];
    let selected = "/Applications/Xcode-26.app/Contents/Developer";
    let git = resolved("/bin:/usr/bin", Some(selected), &files);
    assert_eq!(git, Path::new(&format!("{selected}/usr/bin/git")));
}

#[test]
#[cfg(target_os = "macos")]
fn developer_dir_may_name_the_app() {
    let files = ["/usr/bin/git", "/X.app/Contents/Developer/usr/bin/git"];
    let git = resolved("/usr/bin", Some("/X.app"), &files);
    assert_eq!(git, Path::new("/X.app/Contents/Developer/usr/bin/git"));
}

#[test]
#[cfg(target_os = "macos")]
fn without_a_selection_the_command_line_tools_answer() {
    let files = [
        "/usr/bin/git",
        "/Library/Developer/CommandLineTools/usr/bin/git",
    ];
    let git = resolved("/usr/bin", None, &files);
    assert_eq!(
        git,
        Path::new("/Library/Developer/CommandLineTools/usr/bin/git")
    );
}

#[test]
#[cfg(target_os = "macos")]
fn the_shim_stays_when_nothing_is_behind_it() {
    let git = resolved("/usr/bin", Some("/gone"), &["/usr/bin/git"]);
    assert_eq!(git, Path::new("git"));
}
