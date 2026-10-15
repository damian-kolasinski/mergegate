use std::path::PathBuf;
use std::process::Stdio;

use mergegate_core::diff::DiffGit;
use mergegate_core::git::{Git, Upstream};
use mergegate_core::item::{ChangedFile, ItemGit};
use mergegate_core::project::ProjectName;
use mergegate_git::{HttpRequest, HubGit};
use tempfile::TempDir;
use testkit::{TestRepo, git};

/// A hub with `app` registered and `login` pushed: two commits on top of
/// main's first.
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
        repo.commit_files(
            "Add login",
            &[
                ("src/login.rs", "fn login() {}\n"),
                ("logo.png", "\0\x01\x02"),
            ],
        );
        repo.git(["mv", "README", "README.md"]);
        repo.commit_files(
            "Rename the readme",
            &[("src/login.rs", "fn login() {\n}\n")],
        );
        let hub = Self {
            _dir: dir,
            git,
            staging,
            repo,
        };
        hub.push("login");
        hub
    }

    /// Pushes the checked out branch to staging as `branch`.
    fn push(&self, branch: &str) {
        let dest = format!("HEAD:refs/heads/{branch}");
        self.repo.git([
            "push".as_ref(),
            "--quiet".as_ref(),
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

/// `path`'s blob ID at `rev` in the worker's repo.
fn blob(hub: &Hub, rev: &str, path: &str) -> String {
    let id = hub.repo.git(["rev-parse", &format!("{rev}:{path}")]);
    id.trim().to_owned()
}

fn file(
    path: &str,
    old_path: Option<&str>,
    old: Option<String>,
    new: Option<String>,
) -> ChangedFile {
    ChangedFile {
        path: path.into(),
        old_path: old_path.map(Into::into),
        old_blob: old,
        new_blob: new,
    }
}

#[test]
fn reads_a_pushed_branch_against_its_merge_base() {
    let hub = Hub::new();
    let head = hub.repo.rev("login");
    let base = hub.repo.rev("main");
    assert_eq!(
        hub.git.branch_head(&app(), "login").unwrap(),
        Some(head.clone())
    );
    assert_eq!(hub.git.branch_head(&app(), "nope").unwrap(), None);
    assert_eq!(
        hub.git.merge_base(&app(), &head, "main").unwrap(),
        Some(base.clone())
    );
    assert_eq!(
        hub.git.changed_files(&app(), &base, &head).unwrap(),
        [
            file(
                "README.md",
                Some("README"),
                Some(blob(&hub, "main", "README")),
                Some(blob(&hub, "login", "README.md"))
            ),
            file(
                "logo.png",
                None,
                None,
                Some(blob(&hub, "login", "logo.png"))
            ),
            file(
                "src/login.rs",
                None,
                None,
                Some(blob(&hub, "login", "src/login.rs"))
            ),
        ]
    );
    assert_eq!(
        hub.git
            .first_subject(&app(), &base, &head)
            .unwrap()
            .as_deref(),
        Some("Add login")
    );
}

#[test]
fn reads_blobs_and_nothing_else() {
    let hub = Hub::new();
    let login = blob(&hub, "login", "src/login.rs");
    let logo = blob(&hub, "login", "logo.png");
    let commit = hub.repo.rev("login");
    let missing = "1".repeat(40);
    assert_eq!(
        hub.git
            .blobs(&app(), &[&login, &commit, &logo, &missing, "nope", &login])
            .unwrap(),
        [
            Some(b"fn login() {\n}\n".to_vec()),
            None,
            Some(b"\0\x01\x02".to_vec()),
            None,
            None,
            Some(b"fn login() {\n}\n".to_vec()),
        ]
    );
    assert!(
        hub.git
            .blobs(&ProjectName::parse("web").unwrap(), &[&login])
            .is_err()
    );
}

#[test]
fn finds_no_merge_base_for_unrelated_history() {
    let hub = Hub::new();
    hub.repo.git(["checkout", "--quiet", "--orphan", "lonely"]);
    hub.repo.commit_files("Lonely", &[("x", "y\n")]);
    hub.push("lonely");
    let head = hub.repo.rev("lonely");
    assert_eq!(hub.git.merge_base(&app(), &head, "main").unwrap(), None);
    assert!(hub.git.merge_base(&app(), &head, "nope").is_err());
}

#[test]
fn fetching_a_target_leaves_head_and_other_targets_alone() {
    let hub = Hub::new();
    hub.repo.checkout("main");
    hub.repo.commit_files("Upstream", &[("CHANGELOG", "1\n")]);
    hub.repo.git(["branch", "release"]);

    hub.git
        .fetch_target(&app(), &Upstream::Repo(hub.repo.root().into()), "release")
        .unwrap();
    assert_eq!(hub.staging_rev("release"), hub.repo.rev("release"));
    assert_eq!(
        git(&hub.staging, ["symbolic-ref", "HEAD"]).trim(),
        "refs/heads/main"
    );
    // main is fetched only when asked for.
    assert_ne!(hub.staging_rev("main"), hub.repo.rev("main"));
    hub.git
        .fetch_target(&app(), &Upstream::Repo(hub.repo.root().into()), "main")
        .unwrap();
    assert_eq!(hub.staging_rev("main"), hub.repo.rev("main"));
    assert_eq!(hub.staging_rev("login"), hub.repo.rev("login"));
}

#[test]
fn pins_a_revision() {
    let hub = Hub::new();
    let head = hub.repo.rev("login");
    hub.git
        .pin(&app(), "refs/mergegate/items/1/r1", &head)
        .unwrap();
    assert_eq!(hub.staging_rev("refs/mergegate/items/1/r1"), head);
    let base = hub.repo.rev("main");
    hub.git
        .pin(&app(), "refs/mergegate/items/1/r1", &base)
        .unwrap();
    assert_eq!(hub.staging_rev("refs/mergegate/items/1/r1"), base);
}

#[test]
fn unpins_one_items_revisions() {
    let hub = Hub::new();
    let head = hub.repo.rev("login");
    for reference in [
        "refs/mergegate/items/1/r1",
        "refs/mergegate/items/1/r2",
        "refs/mergegate/items/10/r1",
    ] {
        hub.git.pin(&app(), reference, &head).unwrap();
    }
    hub.git.unpin(&app(), "refs/mergegate/items/1/").unwrap();
    let refs = git(
        &hub.staging,
        ["for-each-ref", "--format=%(refname)", "refs/mergegate"],
    );
    assert_eq!(refs.trim(), "refs/mergegate/items/10/r1");
    assert_eq!(hub.staging_rev("login"), head, "branches stay");
    hub.git.unpin(&app(), "refs/mergegate/items/1/").unwrap();
}

#[test]
fn drops_a_branch_only_where_it_was_left() {
    let hub = Hub::new();
    let head = hub.repo.rev("login");
    let base = hub.repo.rev("main");
    hub.git.drop_branch(&app(), "login", &base).unwrap();
    assert_eq!(hub.staging_rev("login"), head, "it moved on");

    hub.git.drop_branch(&app(), "login", &head).unwrap();
    assert_eq!(hub.git.branch_head(&app(), "login").unwrap(), None);
    hub.git.drop_branch(&app(), "login", &head).unwrap();
    // Its name is free for branches under it.
    hub.push("login/again");
}

fn get(hub: &Hub, path: &str, query: &str) -> String {
    let request = HttpRequest {
        method: "GET",
        path,
        query,
        ..HttpRequest::default()
    };
    let output = hub
        .git
        .http_backend(&app(), &request)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn serves_the_staging_repo_without_the_pins() {
    let hub = Hub::new();
    let head = hub.repo.rev("login");
    hub.git
        .pin(&app(), "refs/mergegate/items/1/r1", &head)
        .unwrap();

    for service in ["git-upload-pack", "git-receive-pack"] {
        let refs = get(&hub, "/info/refs", &format!("service={service}"));
        assert!(
            refs.contains(&format!(
                "Content-Type: application/x-{service}-advertisement"
            )),
            "{refs}"
        );
        assert!(refs.contains("refs/heads/login"), "{refs}");
        assert!(refs.contains("refs/heads/main"), "{refs}");
        assert!(!refs.contains("refs/mergegate"), "{refs}");
    }
    // The dumb protocol would list the pins and serve their objects.
    let dumb = get(&hub, "/info/refs", "");
    assert!(dumb.contains("Status: 403"), "{dumb}");
    assert!(!dumb.contains("refs/mergegate"), "{dumb}");
    let missing = get(&hub, "/nope", "");
    assert!(missing.starts_with("Status: 404"), "{missing}");
}
