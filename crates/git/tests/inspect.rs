use std::path::PathBuf;

use mergegate_core::git::{Git, GitError};
use mergegate_git::HubGit;
use testkit::TestRepo;

fn hub_git() -> HubGit {
    HubGit::new(PathBuf::from("/nonexistent"), PathBuf::from("/nonexistent"))
}

#[test]
fn reads_branches_and_head_from_anywhere_inside_the_repo() {
    let repo = TestRepo::new("app");
    repo.branch("release");
    repo.branch("feature/x");
    repo.checkout("release");
    let deep = repo.root().join("src/deep");
    std::fs::create_dir_all(&deep).unwrap();

    let found = hub_git().inspect(&deep).unwrap();
    assert_eq!(found.root, repo.root());
    assert_eq!(found.branches, ["feature/x", "main", "release"]);
    assert_eq!(found.head.as_deref(), Some("release"));
}

#[test]
fn a_linked_worktree_resolves_to_the_main_repo() {
    let repo = TestRepo::new("app");
    let worktree = repo.worktree("wip");

    let found = hub_git().inspect(&worktree).unwrap();
    assert_eq!(found.root, repo.root());
    assert_eq!(found.branches, ["main", "wip"]);
    assert_eq!(found.head.as_deref(), Some("main"));
}

#[test]
fn a_bare_repo_is_rooted_at_its_git_dir() {
    let repo = TestRepo::new("app");
    let bare = repo.bare_clone("layout/.bare");
    let found = hub_git().inspect(&bare).unwrap();
    assert_eq!(found.root, bare);
    assert_eq!(found.branches, ["main"]);
}

#[test]
fn a_file_inside_the_repo_finds_the_repo() {
    let repo = TestRepo::new("app");
    let file = repo.root().join("README.md");
    std::fs::write(&file, "hi").unwrap();
    assert_eq!(hub_git().inspect(&file).unwrap().root, repo.root());
}

#[test]
fn worktrees_of_a_bare_repo_resolve_to_the_bare_repo() {
    let repo = TestRepo::new("app");
    let bare = repo.bare_clone("layout/.bare");
    let layout = bare.parent().unwrap();
    // The bare-repo layout: `.git` is a file pointing at `.bare`, and each
    // branch is checked out as a linked worktree next to it.
    std::fs::write(layout.join(".git"), "gitdir: ./.bare\n").unwrap();
    let wip = layout.join("wip");
    testkit::git(
        &bare,
        [
            std::ffi::OsStr::new("worktree"),
            "add".as_ref(),
            "--quiet".as_ref(),
            "-b".as_ref(),
            "wip".as_ref(),
            wip.as_os_str(),
        ],
    );

    for path in [layout.to_owned(), wip] {
        let found = hub_git().inspect(&path).unwrap();
        assert_eq!(found.root, bare, "{path:?}");
        assert_eq!(found.branches, ["main", "wip"], "{path:?}");
    }
}

#[test]
fn a_repo_that_cannot_be_read_is_not_reported_as_missing() {
    let repo = TestRepo::new("app");
    std::fs::write(repo.root().join(".git/config"), "[core\n").unwrap();
    let err = hub_git().inspect(repo.root()).unwrap_err();
    assert!(matches!(err, GitError::Failed(_)), "{err:?}");
}

#[test]
fn a_repo_without_commits_has_no_branches() {
    let repo = TestRepo::empty("app");
    let found = hub_git().inspect(repo.root()).unwrap();
    assert!(found.branches.is_empty());
    assert_eq!(found.head.as_deref(), Some("main"));
}

#[test]
fn a_path_outside_any_repo_is_not_a_repo() {
    let dir = tempfile::tempdir().unwrap();
    for path in [dir.path().to_owned(), dir.path().join("missing")] {
        assert!(
            matches!(hub_git().inspect(&path), Err(GitError::NotARepo(p)) if p == path),
            "{path:?}"
        );
    }
}

#[test]
fn lists_remotes_with_their_urls() {
    let repo = TestRepo::new("app");
    repo.git(["remote", "add", "origin", "git@github.com:acme/app.git"]);
    repo.git(["remote", "add", "fork", "https://github.com/me/app"]);

    let found = hub_git().inspect(repo.root()).unwrap();
    let remotes: Vec<_> = found
        .remotes
        .iter()
        .map(|r| (r.name.as_str(), r.url.as_str()))
        .collect();
    assert_eq!(
        remotes,
        [
            ("fork", "https://github.com/me/app"),
            ("origin", "git@github.com:acme/app.git"),
        ]
    );
}
