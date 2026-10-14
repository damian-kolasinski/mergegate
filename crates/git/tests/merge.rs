use std::path::PathBuf;

use mergegate_core::git::{Git, Upstream};
use mergegate_core::item::{DiffStat, Item, ItemBackend, ItemId, ItemState, Revision};
use mergegate_core::merge::{Landing, MergeBackend};
use mergegate_core::project::{Backend, Project, ProjectName};
use mergegate_git::HubGit;
use tempfile::TempDir;
use testkit::{TestRepo, git};

/// The operator's `app` on `main`, with `login` adding a file pushed to
/// staging.
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
        repo.commit_files("Readme", &[("README", "hello\n")]);
        git.sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
            .unwrap();
        repo.git(["checkout", "--quiet", "-b", "login"]);
        repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
        repo.commit_files("Greet", &[("README", "hello, login\n")]);
        let dest = "HEAD:refs/heads/login";
        repo.git([
            "push".as_ref(),
            "--quiet".as_ref(),
            staging.as_os_str(),
            dest.as_ref(),
        ]);
        repo.checkout("main");
        Self {
            _dir: dir,
            git,
            staging,
            repo,
        }
    }

    fn project(&self) -> Project {
        Project {
            name: app(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: self.repo.root().to_owned(),
            github: None,
            bots: Vec::new(),
        }
    }

    fn item(&self) -> Item {
        Item {
            id: ItemId::parse("app#1").unwrap(),
            branch: "login".into(),
            title: "Add login".into(),
            state: ItemState::Merging,
            backend: ItemBackend::Local,
            target: "main".into(),
            revision: Revision {
                number: 1,
                head: self.repo.rev("login"),
                base: self.repo.rev("main"),
                stat: DiffStat::default(),
                submitted_at: String::new(),
            },
            created_at: String::new(),
        }
    }

    fn land(&self) -> Landing {
        let message = "Add login (app#1)";
        self.git
            .land(&self.project(), &self.item(), message)
            .unwrap()
    }
}

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

#[test]
fn squashes_the_item_onto_the_target_as_the_operator() {
    let hub = Hub::new();
    let before = hub.repo.rev("main");
    let Landing::Merged { commit } = hub.land() else {
        panic!("not merged");
    };
    assert_eq!(hub.repo.rev("main"), commit);
    assert_eq!(hub.repo.rev("main^"), before);
    let show = hub
        .repo
        .git(["show", "-s", "--format=%an <%ae>|%cn|%s|%P", "main"]);
    assert_eq!(
        show.trim(),
        format!("Operator <operator@example.com>|Operator|Add login (app#1)|{before}")
    );
    assert_eq!(
        hub.repo.rev("main^{tree}"),
        hub.repo.rev("login^{tree}"),
        "the squash is the branch's tree"
    );
    // The checkout updated in place, and staging follows the target.
    assert_eq!(hub.repo.git(["status", "--porcelain"]), "");
    assert_eq!(
        std::fs::read_to_string(hub.repo.root().join("README")).unwrap(),
        "hello, login\n"
    );
    let staged = git(&hub.staging, ["rev-parse", "refs/heads/main"]);
    assert_eq!(staged.trim(), commit);
}

#[test]
fn a_tree_already_on_the_target_lands_as_the_targets_commit() {
    let hub = Hub::new();
    let Landing::Merged { commit } = hub.land() else {
        panic!("not merged");
    };
    assert_eq!(hub.land(), Landing::Merged { commit });
}

#[test]
fn reports_the_paths_that_conflict_with_the_target() {
    let hub = Hub::new();
    hub.repo
        .commit_files("Greet differently", &[("README", "hi there\n")]);
    let target = hub.repo.rev("main");
    assert_eq!(
        hub.land(),
        Landing::Conflict {
            target_commit: target.clone(),
            paths: vec!["README".into()],
        }
    );
    assert_eq!(hub.repo.rev("main"), target, "the target is untouched");
}

#[test]
fn merges_over_unrelated_commits_on_the_target() {
    let hub = Hub::new();
    hub.repo
        .commit_files("Add docs", &[("docs/guide.md", "Guide\n")]);
    let target = hub.repo.rev("main");
    let Landing::Merged { commit } = hub.land() else {
        panic!("not merged");
    };
    assert_eq!(hub.repo.rev(&format!("{commit}^")), target);
    let files = hub.repo.git(["ls-tree", "-r", "--name-only", "main"]);
    assert_eq!(files, "README\ndocs/guide.md\nsrc/login.rs\n");
}

#[test]
fn a_checked_out_target_with_changes_holds_the_merge() {
    let hub = Hub::new();
    std::fs::write(hub.repo.root().join("README"), "edited\n").unwrap();
    let before = hub.repo.rev("main");
    let Landing::Held { reason } = hub.land() else {
        panic!("not held");
    };
    assert!(reason.contains("refused main"), "{reason}");
    assert_eq!(hub.repo.rev("main"), before);

    hub.repo.git(["checkout", "--", "README"]);
    assert!(matches!(hub.land(), Landing::Merged { .. }));
}

#[test]
fn a_target_checked_out_nowhere_merges_too() {
    let hub = Hub::new();
    hub.repo.git(["checkout", "--quiet", "--detach"]);
    std::fs::write(hub.repo.root().join("scratch"), "wip\n").unwrap();
    let Landing::Merged { commit } = hub.land() else {
        panic!("not merged");
    };
    assert_eq!(hub.repo.rev("main"), commit);
}
