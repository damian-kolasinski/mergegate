use std::path::PathBuf;

use mergegate_core::git::{Git, Upstream};
use mergegate_core::interdiff::InterdiffGit;
use mergegate_core::item::ItemGit;
use mergegate_core::project::ProjectName;
use mergegate_git::HubGit;
use tempfile::TempDir;
use testkit::{TestRepo, git};

/// main's first commit, `login` on it as r1, then main moved on and `login`
/// rebased onto it as r2, each pushed to staging.
struct Hub {
    _dir: TempDir,
    git: HubGit,
    staging: PathBuf,
    repo: TestRepo,
}

impl Hub {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let git = HubGit::new(dir.path().join("projects"), dir.path().join("git-home"));
        let staging = dir.path().join("projects/app/staging.git");
        let repo = TestRepo::new("app");
        repo.commit_files(
            "Initial",
            &[
                ("README", "hello\n"),
                ("lib.rs", "mod lib;\n"),
                ("db.rs", "fn db() {}\n"),
            ],
        );
        git.sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
            .unwrap();
        repo.git(["checkout", "--quiet", "-b", "r1"]);
        repo.commit_files(
            "Add login",
            &[
                ("lib.rs", "mod lib;\nmod login;\n"),
                ("login.rs", "fn login() {}\n"),
            ],
        );
        repo.checkout("main");
        repo.commit_files("Upstream", &[("README", "hello, world\n")]);
        repo.git(["checkout", "--quiet", "-b", "r2"]);
        repo.commit_files(
            "Add login",
            &[
                ("lib.rs", "mod lib;\nmod login;\n"),
                ("login.rs", "fn login() -> bool {}\n"),
            ],
        );
        repo.checkout("main");
        repo.git(["checkout", "--quiet", "-b", "conflicting", "r1"]);
        repo.commit_files("Touch the readme", &[("README", "hi\n")]);
        let hub = Self {
            _dir: dir,
            git,
            staging,
            repo,
        };
        for branch in ["main", "r1", "r2", "conflicting"] {
            hub.push(branch);
        }
        hub
    }

    fn push(&self, branch: &str) {
        let dest = format!("{branch}:refs/heads/{branch}");
        self.repo.git([
            "push".as_ref(),
            "--quiet".as_ref(),
            "--force".as_ref(),
            self.staging.as_os_str(),
            dest.as_ref(),
        ]);
    }

    fn staging_rev(&self, rev: &str) -> String {
        git(&self.staging, ["rev-parse", rev]).trim().to_owned()
    }
}

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

#[test]
fn replays_a_revision_onto_a_moved_base() {
    let hub = Hub::new();
    let (r1, r2) = (hub.repo.rev("r1"), hub.repo.rev("r2"));
    let (base, onto) = (hub.repo.rev("main~1"), hub.repo.rev("main"));

    let replay = hub.git.replay(&app(), &base, &onto, &r1).unwrap();
    assert_eq!(replay.conflicts, Vec::<String>::new());
    let tree = &replay.tree;
    // The target's README, r1's own files.
    assert_eq!(
        hub.staging_rev(&format!("{tree}:README")),
        hub.repo.rev("main:README")
    );
    assert_eq!(
        hub.staging_rev(&format!("{tree}:login.rs")),
        hub.repo.rev("r1:login.rs")
    );

    // From the replayed tree, r2 changes login.rs alone.
    let changed = hub.git.changed_files(&app(), tree, &r2).unwrap();
    let paths: Vec<&str> = changed.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["login.rs"]);
    assert_eq!(
        changed[0].old_blob.as_deref(),
        Some(hub.repo.rev("r1:login.rs").as_str())
    );

    hub.git
        .pin(&app(), "refs/mergegate/items/1/r1-r2", tree)
        .unwrap();
    assert_eq!(hub.staging_rev("refs/mergegate/items/1/r1-r2"), *tree);
}

#[test]
fn lists_the_paths_a_replay_conflicts_on() {
    let hub = Hub::new();
    let head = hub.repo.rev("conflicting");
    let (base, onto) = (hub.repo.rev("main~1"), hub.repo.rev("main"));
    let replay = hub.git.replay(&app(), &base, &onto, &head).unwrap();
    assert_eq!(replay.conflicts, ["README"]);
    let readme = git(
        &hub.staging,
        ["cat-file", "-p", &format!("{}:README", replay.tree)],
    );
    assert!(readme.contains("<<<<<<<"), "{readme}");
}

#[test]
fn a_replay_fails_on_an_unknown_commit() {
    let hub = Hub::new();
    let base = hub.repo.rev("main~1");
    let missing = "1".repeat(40);
    assert!(hub.git.replay(&app(), &base, &missing, &base).is_err());
}
