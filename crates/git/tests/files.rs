use std::path::PathBuf;

use mergegate_core::git::Git;
use mergegate_git::HubGit;
use testkit::TestRepo;

fn hub_git() -> HubGit {
    HubGit::new(PathBuf::from("/nonexistent"), PathBuf::from("/nonexistent"))
}

#[test]
fn lists_every_file_of_the_branch_not_the_checkout() {
    let repo = TestRepo::new("app");
    repo.commit_files(
        "code",
        &[("src/main.rs", "fn main() {}"), ("Cargo.toml", "")],
    );
    repo.git(["checkout", "-q", "-b", "wip"]);
    repo.commit_files("wip", &[("notes.md", "wip")]);

    let mut files = hub_git().files(repo.root(), "main").unwrap();
    files.sort();
    assert!(files.contains(&"src/main.rs".to_owned()), "{files:?}");
    assert!(files.contains(&"Cargo.toml".to_owned()), "{files:?}");
    assert!(!files.contains(&"notes.md".to_owned()), "{files:?}");
}

#[test]
fn a_missing_branch_fails() {
    let repo = TestRepo::new("app");
    assert!(hub_git().files(repo.root(), "nope").is_err());
}
