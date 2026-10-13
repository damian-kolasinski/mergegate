use mergegate_core::git::{Git, parse_version};
use mergegate_core::project::ProjectName;
use mergegate_git::HubGit;
use tempfile::TempDir;
use testkit::{TestRepo, git};

struct Hub {
    dir: TempDir,
    git: HubGit,
}

impl Hub {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let git = HubGit::new(dir.path().join("projects"), dir.path().join("git-home"));
        Self { dir, git }
    }

    fn staging_rev(&self, project: &str, rev: &str) -> String {
        let staging = self
            .dir
            .path()
            .join("projects")
            .join(project)
            .join("staging.git");
        git(&staging, ["rev-parse", rev]).trim().to_owned()
    }
}

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

#[test]
fn creates_staging_and_mirrors_the_target() {
    let hub = Hub::new();
    let repo = TestRepo::new("app");
    assert!(!hub.git.staging_has(&app(), "main").unwrap());

    hub.git.sync_staging(&app(), repo.root(), "main").unwrap();
    assert!(hub.git.staging_has(&app(), "main").unwrap());
    assert!(!hub.git.staging_has(&app(), "release").unwrap());
    assert_eq!(hub.staging_rev("app", "HEAD"), repo.rev("main"));
}

#[test]
fn resyncing_follows_the_target_even_when_history_was_rewritten() {
    let hub = Hub::new();
    let repo = TestRepo::new("app");
    hub.git.sync_staging(&app(), repo.root(), "main").unwrap();

    repo.commit("second");
    hub.git.sync_staging(&app(), repo.root(), "main").unwrap();
    assert_eq!(hub.staging_rev("app", "main"), repo.rev("main"));

    repo.git(["reset", "--quiet", "--hard", "HEAD~1"]);
    repo.commit("rewritten");
    hub.git.sync_staging(&app(), repo.root(), "main").unwrap();
    assert_eq!(hub.staging_rev("app", "main"), repo.rev("main"));
}

#[test]
fn switching_the_target_moves_head() {
    let hub = Hub::new();
    let repo = TestRepo::new("app");
    repo.checkout("main");
    repo.git(["checkout", "--quiet", "-b", "release"]);
    repo.commit("release only");
    hub.git.sync_staging(&app(), repo.root(), "main").unwrap();
    hub.git
        .sync_staging(&app(), repo.root(), "release")
        .unwrap();
    assert_eq!(hub.staging_rev("app", "HEAD"), repo.rev("release"));
}

#[test]
fn a_failed_fetch_says_what_it_was_doing() {
    let hub = Hub::new();
    let repo = TestRepo::new("app");
    let err = hub
        .git
        .sync_staging(&app(), repo.root(), "nope")
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("fetching nope from "), "{err}");
}

#[test]
fn reports_the_git_version() {
    let version = Hub::new().git.version().unwrap();
    assert!(parse_version(&version).is_some(), "{version:?}");
}
