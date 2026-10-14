//! Staging against a repository on GitHub, reached over HTTP with a token.

use mergegate_core::git::{Git, Remote, Secret, Upstream};
use mergegate_core::item::{ItemGit, Pushed};
use mergegate_core::merge::{MergeGit, Trial};
use mergegate_core::project::ProjectName;
use mergegate_git::HubGit;
use tempfile::TempDir;
use testkit::{FakeGithub, TestRepo};

/// `acme/app` on GitHub, cloned from the operator's repo, and a hub whose
/// staging mirrors its `main` and has the agent's `login` pushed.
struct Setup {
    _dir: TempDir,
    git: HubGit,
    github: FakeGithub,
    repo: TestRepo,
}

impl Setup {
    fn new() -> Self {
        let github = FakeGithub::start();
        let repo = TestRepo::new("app");
        repo.commit_files("Readme", &[("README", "hello\n")]);
        github.create_repo("acme/app", repo.root());
        let dir = tempfile::tempdir().unwrap();
        let git = HubGit::new(dir.path().join("projects"), dir.path().join("git-home"));
        let setup = Self {
            _dir: dir,
            git,
            github,
            repo,
        };
        setup
            .git
            .sync_staging(&app(), &setup.upstream(), "main")
            .unwrap();
        setup.repo.git(["checkout", "--quiet", "-b", "login"]);
        setup
            .repo
            .commit_files("Add login", &[("login.rs", "fn login() {}\n")]);
        setup.stage("login");
        setup
    }

    /// Pushes the operator's `branch` to staging, as `mg submit` does.
    fn stage(&self, branch: &str) {
        let staging = self._dir.path().join("projects/app/staging.git");
        self.repo.git([
            "push".as_ref(),
            "--quiet".as_ref(),
            "--force".as_ref(),
            staging.as_os_str(),
            format!("{branch}:refs/heads/{branch}").as_ref(),
        ]);
    }

    fn remote(&self) -> Remote {
        let (url, auth) = self.github.remote("acme/app");
        Remote {
            url,
            auth: Secret::new(auth),
        }
    }

    fn upstream(&self) -> Upstream {
        Upstream::Remote(self.remote())
    }
}

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

#[test]
fn fetches_the_target_only_when_signed_in() {
    let setup = Setup::new();
    setup.repo.checkout("main");
    setup.repo.commit("upstream moved");
    setup.github.git(
        "acme/app",
        [
            "fetch".as_ref(),
            "--quiet".as_ref(),
            setup.repo.root().as_os_str(),
            "main:main".as_ref(),
        ],
    );
    setup
        .git
        .fetch_target(&app(), &setup.upstream(), "main")
        .unwrap();
    assert_eq!(
        setup.git.branch_head(&app(), "main").unwrap(),
        Some(setup.repo.rev("main"))
    );

    let stale = setup.remote();
    setup.github.log_in("gho_other");
    let err = setup
        .git
        .fetch_target(&app(), &Upstream::Remote(stale), "main")
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("fetching main from http://"), "{err}");
    // The header never shows up in an error.
    assert!(!err.contains("Basic"), "{err}");
}

#[test]
fn pushes_a_branch_only_from_where_it_last_pushed_it() {
    let setup = Setup::new();
    let remote = setup.remote();
    let head = setup.repo.rev("login");
    let main = setup.repo.rev("main");

    let pushed = setup.git.push_branch(&app(), &remote, "login", &head, None);
    assert_eq!(pushed.unwrap(), Pushed::Done);
    assert_eq!(setup.github.rev("acme/app", "login"), Some(head.clone()));
    // Already there, as after a submit that failed later.
    let again = setup.git.push_branch(&app(), &remote, "login", &head, None);
    assert_eq!(again.unwrap(), Pushed::Done);

    // A force push from where it left the branch goes through.
    let back = setup.git.push_branch(&app(), &remote, "login", &main, None);
    assert_eq!(back.unwrap(), Pushed::Done);
    assert_eq!(setup.github.rev("acme/app", "login"), Some(main.clone()));

    // Someone else pushed to it on GitHub.
    setup.github.git(
        "acme/app",
        ["update-ref", "refs/heads/login", head.as_str()],
    );
    let moved = setup.git.push_branch(&app(), &remote, "login", &main, None);
    assert_eq!(moved.unwrap(), Pushed::Moved(head.clone()));
    assert_eq!(setup.github.rev("acme/app", "login"), Some(head.clone()));

    // Deleted under an open pull request, as closing it on GitHub may do:
    // left alone. Deleted as a merge does: pushing creates it again.
    setup
        .github
        .git("acme/app", ["update-ref", "-d", "refs/heads/login"]);
    let closed = setup
        .git
        .push_branch(&app(), &remote, "login", &main, Some(&main));
    assert_eq!(closed.unwrap(), Pushed::Gone);
    assert_eq!(setup.github.rev("acme/app", "login"), None);
    let gone = setup.git.push_branch(&app(), &remote, "login", &main, None);
    assert_eq!(gone.unwrap(), Pushed::Done);
    assert_eq!(setup.github.rev("acme/app", "login"), Some(main));
}

#[test]
fn pushes_a_branch_named_under_one_it_pushed_before() {
    let setup = Setup::new();
    let remote = setup.remote();
    let head = setup.repo.rev("login");
    let pushed = setup.git.push_branch(&app(), &remote, "login", &head, None);
    assert_eq!(pushed.unwrap(), Pushed::Done);
    setup
        .github
        .git("acme/app", ["update-ref", "-d", "refs/heads/login"]);

    let nested = setup
        .git
        .push_branch(&app(), &remote, "login/retry", &head, None);
    assert_eq!(nested.unwrap(), Pushed::Done);
    assert_eq!(setup.github.rev("acme/app", "login/retry"), Some(head));
}

#[test]
fn never_replaces_a_branch_it_never_pushed() {
    let setup = Setup::new();
    let main = setup.repo.rev("main");
    let head = setup.repo.rev("login");
    setup.github.git(
        "acme/app",
        [
            "fetch".as_ref(),
            "--quiet".as_ref(),
            setup.repo.root().as_os_str(),
            "login:login".as_ref(),
        ],
    );
    let pushed = setup
        .git
        .push_branch(&app(), &setup.remote(), "login", &main, None);
    assert_eq!(pushed.unwrap(), Pushed::Moved(head.clone()));
    assert_eq!(setup.github.rev("acme/app", "login"), Some(head.clone()));

    // Unless an open item pushed it there, and staging lost the record.
    let item = setup
        .git
        .push_branch(&app(), &setup.remote(), "login", &main, Some(&head));
    assert_eq!(item.unwrap(), Pushed::Done);
    assert_eq!(setup.github.rev("acme/app", "login"), Some(main));
}

#[test]
fn builds_on_a_push_it_didnt_make() {
    let setup = Setup::new();
    let remote = setup.remote();
    let head = setup.repo.rev("login");
    let pushed = setup.git.push_branch(&app(), &remote, "login", &head, None);
    assert_eq!(pushed.unwrap(), Pushed::Done);

    // Someone else pushes a fix to the branch on GitHub.
    setup.repo.commit_files(
        "Fix login",
        &[("login.rs", "fn login() -> bool { true }\n")],
    );
    let fix = setup.repo.rev("login");
    setup.github.git(
        "acme/app",
        [
            "fetch".as_ref(),
            "--quiet".as_ref(),
            setup.repo.root().as_os_str(),
            "login:login".as_ref(),
        ],
    );

    // A rebase that drops it is stopped.
    setup
        .repo
        .git(["reset", "--quiet", "--hard", head.as_str()]);
    setup
        .repo
        .commit_files("Log out", &[("logout.rs", "fn logout() {}\n")]);
    setup.stage("login");
    let dropped = setup.repo.rev("login");
    let moved = setup
        .git
        .push_branch(&app(), &remote, "login", &dropped, None);
    assert_eq!(moved.unwrap(), Pushed::Moved(fix.clone()));
    assert_eq!(setup.github.rev("acme/app", "login"), Some(fix.clone()));

    // The agent fetches the fix and builds on it: that goes through.
    setup.repo.git(["reset", "--quiet", "--hard", fix.as_str()]);
    setup
        .repo
        .commit_files("Log out", &[("logout.rs", "fn logout() {}\n")]);
    setup.stage("login");
    let built = setup.repo.rev("login");
    let pushed = setup
        .git
        .push_branch(&app(), &remote, "login", &built, None);
    assert_eq!(pushed.unwrap(), Pushed::Done);
    assert_eq!(setup.github.rev("acme/app", "login"), Some(built));
}

#[test]
fn a_push_without_access_fails() {
    let setup = Setup::new();
    setup.github.deny_push("acme/app");
    let head = setup.repo.rev("login");
    let err = setup
        .git
        .push_branch(&app(), &setup.remote(), "login", &head, None)
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("pushing login to http://"), "{err}");
}

#[test]
fn trial_merges_against_the_fetched_target() {
    let setup = Setup::new();
    let head = setup.repo.rev("login");
    let trial = setup
        .git
        .trial_merge(&app(), &setup.upstream(), "main", &head)
        .unwrap();
    assert_eq!(trial, Trial::Clean);

    // `main` on GitHub now adds the same file differently.
    setup.repo.checkout("main");
    setup
        .repo
        .commit_files("Other login", &[("login.rs", "fn other() {}\n")]);
    setup.github.git(
        "acme/app",
        [
            "fetch".as_ref(),
            "--quiet".as_ref(),
            setup.repo.root().as_os_str(),
            "main:main".as_ref(),
        ],
    );
    let trial = setup
        .git
        .trial_merge(&app(), &setup.upstream(), "main", &head)
        .unwrap();
    assert_eq!(
        trial,
        Trial::Conflict {
            target_commit: setup.repo.rev("main"),
            paths: vec!["login.rs".into()],
        }
    );
}
