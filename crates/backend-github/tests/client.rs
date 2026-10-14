use mergegate_backend_github::GithubClient;
use mergegate_core::github::{Github, GithubError, GithubRepo, Pull};
use testkit::{FakeGithub, GithubPullState, TestRepo, git};

/// GitHub with `acme/app`, whose `login` branch adds `login.rs` on `main`.
fn setup() -> (FakeGithub, GithubClient, TestRepo) {
    let github = FakeGithub::start();
    let repo = TestRepo::new("app");
    repo.git(["checkout", "--quiet", "-b", "login"]);
    repo.commit_files("Add login", &[("login.rs", "fn login() {}\n")]);
    repo.checkout("main");
    github.create_repo("acme/app", repo.root());
    let client = GithubClient::new(github.api_url(), github.web_url(), github.gh());
    (github, client, repo)
}

fn acme() -> GithubRepo {
    GithubRepo::parse("acme/app").unwrap()
}

#[test]
fn checks_the_account_can_push() {
    let (github, client, _repo) = setup();
    client.check_access(&acme()).unwrap();

    let missing = GithubRepo::parse("acme/nope").unwrap();
    let err = client.check_access(&missing).unwrap_err();
    assert!(matches!(err, GithubError::NoAccess(_)), "{err:?}");

    github.deny_push("acme/app");
    let err = client.check_access(&acme()).unwrap_err();
    assert!(matches!(err, GithubError::NoAccess(_)), "{err:?}");
}

#[test]
fn signs_in_with_the_token_gh_gives() {
    let (github, client, _repo) = setup();
    client.check_access(&acme()).unwrap();
    // A new login takes effect without a new client.
    github.log_in("gho_second");
    client.check_access(&acme()).unwrap();

    github.log_out();
    let err = client.check_access(&acme()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "the hub can't sign in to GitHub: gh auth token failed: \
         no oauth token found for github.com"
    );
    // A token sent only as a bearer token, never in a URL.
    assert!(github.requests().iter().all(|r| !r.contains("gho_")));
}

#[test]
fn a_missing_gh_says_so() {
    let github = FakeGithub::start();
    let client = GithubClient::new(
        github.api_url(),
        github.web_url(),
        github.bin().join("nope"),
    );
    let err = client.check_access(&acme()).unwrap_err().to_string();
    assert!(
        err.starts_with("the hub can't sign in to GitHub: can't run gh"),
        "{err}"
    );
}

#[test]
fn opens_finds_retitles_and_closes_a_pull_request() {
    let (github, client, _repo) = setup();
    assert_eq!(client.find_pull(&acme(), "login", "main").unwrap(), None);

    let pull = client
        .open_pull(&acme(), "login", "main", "Add login")
        .unwrap();
    let expected = Pull {
        number: 1,
        url: format!("{}/acme/app/pull/1", github.web_url()),
    };
    assert_eq!(pull, expected);
    assert_eq!(
        client.find_pull(&acme(), "login", "main").unwrap(),
        Some(expected)
    );
    // Only one into the item's target is its pull request.
    assert_eq!(client.find_pull(&acme(), "login", "release").unwrap(), None);
    let [opened] = &github.pulls()[..] else {
        panic!("{:?}", github.pulls());
    };
    assert_eq!(
        (opened.body.as_str(), opened.draft, opened.base.as_str()),
        ("", false, "main")
    );

    let err = client
        .open_pull(&acme(), "login", "main", "Again")
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "GitHub refused: Validation Failed: A pull request already exists for acme:login."
    );

    client.retitle_pull(&acme(), 1, "Sign in").unwrap();
    assert_eq!(github.pulls()[0].title, "Sign in");
    client.close_pull(&acme(), 1).unwrap();
    assert_eq!(github.pulls()[0].state, GithubPullState::Closed);
    assert_eq!(client.find_pull(&acme(), "login", "main").unwrap(), None);
}

#[test]
fn squash_merges_only_the_approved_head() {
    let (github, client, repo) = setup();
    client
        .open_pull(&acme(), "login", "main", "Add login")
        .unwrap();
    let head = repo.rev("login");
    assert_eq!(client.merged(&acme(), 1, &head).unwrap(), None);

    let err = client
        .squash_merge(&acme(), 1, "Add login (app#1)", &repo.rev("main"))
        .unwrap_err();
    assert!(
        matches!(&err, GithubError::Refused(m) if m.starts_with("Head branch was modified")),
        "{err:?}"
    );

    let commit = client
        .squash_merge(&acme(), 1, "Add login (app#1)", &head)
        .unwrap();
    assert_eq!(
        github.rev("acme/app", "main").as_deref(),
        Some(commit.as_str())
    );
    assert_eq!(
        github.git("acme/app", ["log", "-1", "--format=%s%n%b|%an", "main"]),
        "Add login (app#1)\n|Operator\n"
    );
    assert_eq!(github.rev("acme/app", "main^").unwrap(), repo.rev("main"));

    assert_eq!(
        client.merged(&acme(), 1, &head).unwrap(),
        Some(commit.clone())
    );
    assert_eq!(client.merged(&acme(), 1, &repo.rev("main")).unwrap(), None);
    // A retry after the merge went through finds it.
    assert_eq!(
        client
            .squash_merge(&acme(), 1, "Add login (app#1)", &head)
            .unwrap(),
        commit
    );
}

#[test]
fn a_refused_merge_says_why() {
    let (github, client, repo) = setup();
    client
        .open_pull(&acme(), "login", "main", "Add login")
        .unwrap();
    github.refuse_merges(Some("Required status check \"ci\" is expected."));
    let err = client
        .squash_merge(&acme(), 1, "Add login (app#1)", &repo.rev("login"))
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "GitHub refused: Required status check \"ci\" is expected."
    );
    assert_eq!(github.rev("acme/app", "main").unwrap(), repo.rev("main"));
}

#[test]
fn deletes_a_branch_even_one_already_gone() {
    let (github, client, _repo) = setup();
    client.delete_branch(&acme(), "login").unwrap();
    assert_eq!(github.rev("acme/app", "login"), None);
    client.delete_branch(&acme(), "login").unwrap();
}

#[test]
fn git_reaches_the_repository_with_the_remote() {
    let (_github, client, repo) = setup();
    let remote = client.remote(&acme()).unwrap();
    assert!(!format!("{:?}", remote.auth).contains("Basic"));
    let config = format!("http.{}.extraheader={}", remote.url, remote.auth.expose());
    let refs = git(
        repo.root(),
        [
            "-c",
            config.as_str(),
            "ls-remote",
            remote.url.as_str(),
            "refs/heads/login",
        ],
    );
    assert!(refs.starts_with(&repo.rev("login")), "{refs}");
}
