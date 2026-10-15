use mergegate_core::conflict::ConflictGit;
use mergegate_core::git::{Git, Upstream};
use mergegate_core::item::ItemGit;
use mergegate_core::project::ProjectName;
use mergegate_git::HubGit;
use testkit::TestRepo;

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

/// Pushes a branch off main that writes `files` to staging, and returns its
/// head.
fn branch(
    repo: &TestRepo,
    staging: &std::path::Path,
    name: &str,
    files: &[(&str, &str)],
) -> String {
    repo.git(["checkout", "--quiet", "-b", name, "main"]);
    repo.commit_files(name, files);
    let dest = format!("HEAD:refs/heads/{name}");
    repo.git([
        "push".as_ref(),
        "--quiet".as_ref(),
        staging.as_os_str(),
        dest.as_ref(),
    ]);
    repo.checkout("main");
    repo.rev(name)
}

#[test]
fn names_the_paths_two_heads_conflict_on() {
    let dir = tempfile::tempdir().unwrap();
    let git = HubGit::new(dir.path().join("projects"), dir.path().join("git-home"));
    let staging = dir.path().join("projects/app/staging.git");
    let repo = TestRepo::new("app");
    repo.commit_files("Readme", &[("README", "hello\n"), ("LICENSE", "MIT\n")]);
    git.sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
        .unwrap();
    let greet = branch(&repo, &staging, "greet", &[("README", "hi\n")]);
    let wave = branch(
        &repo,
        &staging,
        "wave",
        &[("README", "hey\n"), ("LICENSE", "BSD\n")],
    );
    let docs = branch(&repo, &staging, "docs", &[("docs/guide.md", "Guide\n")]);

    assert_eq!(git.conflicts(&app(), &greet, &wave).unwrap(), ["README"]);
    assert_eq!(git.conflicts(&app(), &wave, &greet).unwrap(), ["README"]);
    assert!(git.conflicts(&app(), &greet, &docs).unwrap().is_empty());
    let main = repo.rev("main");
    assert!(git.conflicts(&app(), &main, &wave).unwrap().is_empty());
}

#[test]
fn the_watch_fetches_aside_and_only_fast_forwards_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let git = HubGit::new(dir.path().join("projects"), dir.path().join("git-home"));
    let repo = TestRepo::new("app");
    repo.commit_files("Readme", &[("README", "hello\n")]);
    let upstream = Upstream::Repo(repo.root().into());
    git.sync_staging(&app(), &upstream, "main").unwrap();
    let old = repo.rev("main");
    repo.commit_files("More", &[("README", "hi\n")]);
    let new = repo.rev("main");

    assert_eq!(git.fetch_watched(&app(), &upstream, "main").unwrap(), new);
    assert_eq!(git.branch_head(&app(), "main").unwrap(), Some(old.clone()));
    git.advance(&app(), "main", &new).unwrap();
    assert_eq!(git.branch_head(&app(), "main").unwrap(), Some(new.clone()));
    // Back to an older commit isn't a fast-forward: the target stays.
    git.advance(&app(), "main", &old).unwrap();
    assert_eq!(git.branch_head(&app(), "main").unwrap(), Some(new));
}
