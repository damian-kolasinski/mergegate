use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A real git repository in its own temporary directory, with `main` holding
/// one commit. Other paths made through it (worktrees, bare clones) live in
/// the same temporary directory.
pub struct TestRepo {
    dir: TempDir,
    root: PathBuf,
}

impl TestRepo {
    /// A repo in a folder called `name`, so names derived from it are known.
    pub fn new(name: &str) -> Self {
        let repo = Self::empty(name);
        repo.commit("initial");
        repo
    }

    /// A repo with no commits, and so no branches.
    pub fn empty(name: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("mg-repo-")
            .tempdir()
            .expect("create repo dir");
        // Canonical, so it compares equal to the roots the hub reports
        // (on macOS the temp dir is behind a /var -> /private/var link).
        let root = dir.path().canonicalize().unwrap().join(name);
        git(
            dir.path(),
            ["init", "--quiet", "--initial-branch=main", name],
        );
        Self { dir, root }
    }

    /// The main worktree.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A path next to the repo, inside the same temporary directory.
    pub fn sibling(&self, name: &str) -> PathBuf {
        self.root.with_file_name(name)
    }

    /// Runs git in the main worktree and returns its stdout.
    #[track_caller]
    pub fn git<I, S>(&self, args: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        git(&self.root, args)
    }

    pub fn commit(&self, message: &str) {
        self.git(["commit", "--quiet", "--allow-empty", "-m", message]);
    }

    pub fn branch(&self, name: &str) {
        self.git(["branch", name]);
    }

    pub fn checkout(&self, branch: &str) {
        self.git(["checkout", "--quiet", branch]);
    }

    /// The commit `rev` points at.
    pub fn rev(&self, rev: &str) -> String {
        self.git(["rev-parse", rev]).trim().to_owned()
    }

    /// Adds a linked worktree on a new branch `name`, next to the repo.
    pub fn worktree(&self, name: &str) -> PathBuf {
        let path = self.sibling(name);
        self.git([
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--quiet"),
            OsStr::new("-b"),
            OsStr::new(name),
            path.as_os_str(),
        ]);
        path
    }

    /// A bare clone at `rel`, a path relative to the temporary directory.
    pub fn bare_clone(&self, rel: &str) -> PathBuf {
        let path = self.dir.path().canonicalize().unwrap().join(rel);
        self.git([
            OsStr::new("clone"),
            OsStr::new("--quiet"),
            OsStr::new("--bare"),
            self.root.as_os_str(),
            path.as_os_str(),
        ]);
        path
    }
}

/// Runs git in `dir` with none of the developer's git settings and a fixed
/// identity, and returns its stdout.
#[track_caller]
pub fn git<I, S>(dir: &Path, args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.as_encoded_bytes().starts_with(b"GIT_") {
            cmd.env_remove(key);
        }
    }
    cmd.current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .args(args);
    let output = cmd.output().expect("run git");
    assert!(
        output.status.success(),
        "{cmd:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}
