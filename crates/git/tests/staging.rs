use mergegate_core::git::{Git, Upstream, parse_version};
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

    hub.git
        .sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
        .unwrap();
    assert!(hub.git.staging_has(&app(), "main").unwrap());
    assert!(!hub.git.staging_has(&app(), "release").unwrap());
    assert_eq!(hub.staging_rev("app", "HEAD"), repo.rev("main"));
}

#[test]
fn resyncing_follows_the_target_even_when_history_was_rewritten() {
    let hub = Hub::new();
    let repo = TestRepo::new("app");
    hub.git
        .sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
        .unwrap();

    repo.commit("second");
    hub.git
        .sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
        .unwrap();
    assert_eq!(hub.staging_rev("app", "main"), repo.rev("main"));

    repo.git(["reset", "--quiet", "--hard", "HEAD~1"]);
    repo.commit("rewritten");
    hub.git
        .sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
        .unwrap();
    assert_eq!(hub.staging_rev("app", "main"), repo.rev("main"));
}

#[test]
fn switching_the_target_moves_head() {
    let hub = Hub::new();
    let repo = TestRepo::new("app");
    repo.checkout("main");
    repo.git(["checkout", "--quiet", "-b", "release"]);
    repo.commit("release only");
    hub.git
        .sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
        .unwrap();
    hub.git
        .sync_staging(&app(), &Upstream::Repo(repo.root().into()), "release")
        .unwrap();
    assert_eq!(hub.staging_rev("app", "HEAD"), repo.rev("release"));
    // The old target would only go stale.
    assert!(!hub.git.staging_has(&app(), "main").unwrap());
}

#[test]
fn a_new_target_may_reuse_the_old_ones_path() {
    let hub = Hub::new();
    let repo = TestRepo::new("app");
    repo.git(["branch", "-m", "main", "release/1.0"]);
    hub.git
        .sync_staging(&app(), &Upstream::Repo(repo.root().into()), "release/1.0")
        .unwrap();

    // `release` can't sit next to `release/1.0`, so the old target must go first.
    repo.git(["branch", "-m", "release/1.0", "release"]);
    hub.git
        .sync_staging(&app(), &Upstream::Repo(repo.root().into()), "release")
        .unwrap();
    assert_eq!(hub.staging_rev("app", "HEAD"), repo.rev("release"));
}

#[test]
fn mirrors_a_shallow_upstream() {
    let hub = Hub::new();
    let repo = TestRepo::new("app");
    repo.commit("second");
    let shallow = repo.sibling("shallow");
    let mut url = std::ffi::OsString::from("file://");
    url.push(repo.root());
    testkit::git(
        repo.root(),
        [
            "clone".as_ref(),
            "--quiet".as_ref(),
            "--depth=1".as_ref(),
            url.as_os_str(),
            shallow.as_os_str(),
        ],
    );

    hub.git
        .sync_staging(&app(), &Upstream::Repo(shallow.clone()), "main")
        .unwrap();
    assert_eq!(hub.staging_rev("app", "main"), repo.rev("main"));
}

#[test]
fn a_failed_fetch_says_what_it_was_doing() {
    let hub = Hub::new();
    let repo = TestRepo::new("app");
    let err = hub
        .git
        .sync_staging(&app(), &Upstream::Repo(repo.root().into()), "nope")
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("fetching nope from "), "{err}");
    // A first sync that failed leaves nothing that looks like staging.
    assert!(!hub.dir.path().join("projects/app/staging.git").exists());
}

#[test]
fn a_broken_staging_dir_is_an_error_and_a_resync_repairs_it() {
    let hub = Hub::new();
    let repo = TestRepo::new("app");
    let staging = hub.dir.path().join("projects/app/staging.git");
    std::fs::create_dir_all(&staging).unwrap();

    assert!(hub.git.staging_has(&app(), "main").is_err());
    hub.git
        .sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
        .unwrap();
    assert!(hub.git.staging_has(&app(), "main").unwrap());
}

#[test]
fn reports_the_git_version() {
    let version = Hub::new().git.version().unwrap();
    assert!(parse_version(&version).is_some(), "{version:?}");
}
