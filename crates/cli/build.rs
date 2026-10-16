//! Sets `MG_VERSION`: the crate version, plus the commit mg was built from
//! when that isn't the release's tag, as for `cargo install --git` builds
//! of `main`. Every build between two releases shares the crate version, so
//! without the commit a hub left running across an upgrade looks current. A
//! release reports its version alone, as its tag names it. The hub only
//! hears it from the CLI that starts it: watching git from the hub's own
//! build script would rebuild the UI on every commit.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let crate_version = std::env::var("CARGO_PKG_VERSION").expect("set by cargo");
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo");
    let version = match commit(Path::new(&dir), &format!("v{crate_version}")) {
        Some(commit) => format!("{crate_version}+{commit}"),
        None => crate_version,
    };
    println!("cargo::rustc-env=MG_VERSION={version}");
}

/// The checkout's commit, unless it is `release`'s, watching what moves it:
/// HEAD, and the branch HEAD names, loose or packed.
fn commit(dir: &Path, release: &str) -> Option<String> {
    let tags = git(dir, &["tag", "--points-at", "HEAD"]).unwrap_or_default();
    if tags.lines().any(|tag| tag == release) {
        return None;
    }
    let commit = git(dir, &["rev-parse", "--short=12", "HEAD"])?;
    let mut watched = vec!["HEAD".to_owned(), "packed-refs".to_owned()];
    watched.extend(git(dir, &["symbolic-ref", "-q", "HEAD"]));
    for name in watched {
        // Absolute, since a worktree's HEAD lives outside the checkout.
        let path = git(
            dir,
            &["rev-parse", "--path-format=absolute", "--git-path", &name],
        );
        // A missing path would rerun this script on every build.
        if let Some(path) = path.filter(|p| Path::new(p).exists()) {
            println!("cargo::rerun-if-changed={path}");
        }
    }
    Some(commit)
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    let out = String::from_utf8(output.stdout).ok()?;
    let out = out.trim();
    (output.status.success() && !out.is_empty()).then(|| out.to_owned())
}
