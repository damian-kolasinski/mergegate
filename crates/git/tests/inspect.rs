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
