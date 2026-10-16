use mergegate_backend_github::GithubClient;
use mergegate_core::github::{
    Github, GithubError, GithubRepo, Pull, PullComment, PullCommentKind, PullLines,
};
use testkit::{FakeGithub, GithubNewComment, GithubPullState, TestRepo, git};

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
fn opens_finds_edits_and_closes_a_pull_request() {
    let (github, client, _repo) = setup();
    assert_eq!(client.find_pull(&acme(), "login", "main").unwrap(), None);

    let pull = client
        .open_pull(&acme(), "login", "main", "Add login", "Signs users in.")
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
        ("Signs users in.", false, "main")
    );

    let err = client
        .open_pull(&acme(), "login", "main", "Again", "")
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "GitHub refused: Validation Failed: A pull request already exists for acme:login."
    );

    client
        .edit_pull(&acme(), 1, "Sign in", Some("Signs users in, safely."))
        .unwrap();
    let edited = &github.pulls()[0];
    assert_eq!(
        (edited.title.as_str(), edited.body.as_str()),
        ("Sign in", "Signs users in, safely.")
    );
    assert_eq!(
        client.pull_body(&acme(), 1).unwrap(),
        "Signs users in, safely."
    );
    // Without a description, only the title changes.
    client.edit_pull(&acme(), 1, "Sign in again", None).unwrap();
    let retitled = &github.pulls()[0];
    assert_eq!(
        (retitled.title.as_str(), retitled.body.as_str()),
        ("Sign in again", "Signs users in, safely.")
    );
    client.close_pull(&acme(), 1).unwrap();
    assert_eq!(github.pulls()[0].state, GithubPullState::Closed);
    assert_eq!(client.find_pull(&acme(), "login", "main").unwrap(), None);
}

#[test]
fn squash_merges_only_the_approved_head() {
    let (github, client, repo) = setup();
    client
        .open_pull(&acme(), "login", "main", "Add login", "")
        .unwrap();
    let head = repo.rev("login");
    assert_eq!(client.merged(&acme(), 1, &head).unwrap(), None);

    let err = client
        .squash_merge(&acme(), 1, "Add login (app#1)", "", &repo.rev("main"))
        .unwrap_err();
    assert!(
        matches!(&err, GithubError::Refused(m) if m.starts_with("Head branch was modified")),
        "{err:?}"
    );

    let commit = client
        .squash_merge(&acme(), 1, "Add login (app#1)", "Signs users in.", &head)
        .unwrap();
    assert_eq!(
        github.rev("acme/app", "main").as_deref(),
        Some(commit.as_str())
    );
    assert_eq!(
        github.git("acme/app", ["log", "-1", "--format=%s%n%b|%an", "main"]),
        "Add login (app#1)\nSigns users in.\n|Operator\n"
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
            .squash_merge(&acme(), 1, "Add login (app#1)", "", &head)
            .unwrap(),
        commit
    );
}

#[test]
fn a_refused_merge_says_why() {
    let (github, client, repo) = setup();
    client
        .open_pull(&acme(), "login", "main", "Add login", "")
        .unwrap();
    github.refuse_merges(Some("Required status check \"ci\" is expected."));
    let err = client
        .squash_merge(&acme(), 1, "Add login (app#1)", "", &repo.rev("login"))
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "GitHub refused: Required status check \"ci\" is expected."
    );
    assert_eq!(github.rev("acme/app", "main").unwrap(), repo.rev("main"));
}

#[test]
fn retargets_a_pull_request_and_one_on_a_deleted_branch_closes() {
    let github = FakeGithub::start();
    let repo = TestRepo::new("app");
    repo.git(["checkout", "--quiet", "-b", "login"]);
    repo.commit_files("Add login", &[("login.rs", "fn login() {}\n")]);
    repo.git(["checkout", "--quiet", "-b", "profile"]);
    repo.commit_files("Add profile", &[("profile.rs", "fn profile() {}\n")]);
    repo.git(["checkout", "--quiet", "-b", "avatar"]);
    repo.commit_files("Add avatar", &[("avatar.rs", "fn avatar() {}\n")]);
    repo.checkout("main");
    github.create_repo("acme/app", repo.root());
    let client = GithubClient::new(github.api_url(), github.web_url(), github.gh());
    client
        .open_pull(&acme(), "profile", "login", "Add profile", "")
        .unwrap();
    client
        .open_pull(&acme(), "avatar", "profile", "Add avatar", "")
        .unwrap();

    client.retarget(&acme(), 1, "main").unwrap();
    assert_eq!(github.pulls()[0].base, "main");
    let err = client.retarget(&acme(), 1, "nope").unwrap_err();
    assert!(err.to_string().contains("base nope doesn't exist"), "{err}");

    // GitHub closes what's based on a branch it deletes, for good: the
    // hub retargets a child before it deletes its parent's branch.
    client.delete_branch(&acme(), "profile").unwrap();
    assert_eq!(github.pulls()[1].state, GithubPullState::Closed);
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

/// Pull request 1, `login` into `main`.
fn with_pull() -> (FakeGithub, GithubClient, TestRepo) {
    let (github, client, repo) = setup();
    client
        .open_pull(&acme(), "login", "main", "Add login", "")
        .unwrap();
    (github, client, repo)
}

#[test]
fn reads_who_is_signed_in_once() {
    let (github, client, _repo) = setup();
    assert_eq!(client.viewer().unwrap(), "operator");
    github.set_viewer("someone");
    assert_eq!(client.viewer().unwrap(), "operator");
    let asked = github
        .requests()
        .iter()
        .filter(|r| *r == "GET /api/user")
        .count();
    assert_eq!(asked, 1);
}

#[test]
fn lists_every_kind_of_comment_on_a_pull_request() {
    let (github, client, _repo) = with_pull();
    let head = github.rev("acme/app", "login").unwrap();
    let inline = GithubNewComment::inline("codex[bot]", "Leaks", "login.rs", None, 1);
    let inline = github.comment("acme/app", 1, inline);
    let issue = github.comment("acme/app", 1, GithubNewComment::issue("ana", "Why?"));
    let review = github.comment("acme/app", 1, GithubNewComment::review("ana", "Close"));

    let dated = |id| {
        let comment = github.comments().into_iter().find(|c| c.id == id);
        comment.unwrap().updated_at * 1_000
    };
    let comments = client.pull_comments(&acme(), 1, false).unwrap();
    let page = format!("{}/acme/app/pull/1", github.web_url());
    let expected = vec![
        PullComment {
            kind: PullCommentKind::ReviewComment,
            id: inline,
            login: "codex[bot]".into(),
            bot: true,
            url: format!("{page}#discussion_r{inline}"),
            body: "Leaks".into(),
            lines: Some(PullLines {
                path: "login.rs".into(),
                old_side: false,
                original_commit: head.clone(),
                original_start_line: None,
                original_line: Some(1),
                commit: head.clone(),
                start_line: None,
                line: Some(1),
            }),
            dated: dated(inline),
        },
        PullComment {
            kind: PullCommentKind::IssueComment,
            id: issue,
            login: "ana".into(),
            bot: false,
            url: format!("{page}#issuecomment-{issue}"),
            body: "Why?".into(),
            lines: None,
            dated: dated(issue),
        },
        PullComment {
            kind: PullCommentKind::Review,
            id: review,
            login: "ana".into(),
            bot: false,
            url: format!("{page}#pullrequestreview-{review}"),
            body: "Close".into(),
            lines: None,
            dated: dated(review),
        },
    ];
    assert_eq!(comments, expected);

    // A push outdates the inline comment: it keeps where it was written.
    github.git("acme/app", ["update-ref", "refs/heads/login", "main"]);
    let comments = client.pull_comments(&acme(), 1, false).unwrap();
    let lines = comments[0].lines.clone().unwrap();
    assert_eq!(
        (lines.original_commit, lines.original_line, lines.line),
        (head, Some(1), None)
    );
}

#[test]
fn asks_again_with_the_etag_unless_fresh() {
    let (github, client, _repo) = with_pull();
    let id = github.comment("acme/app", 1, GithubNewComment::issue("ana", "Why?"));
    let first = client.pull_comments(&acme(), 1, false).unwrap();
    assert_eq!(github.not_modified(), 0);

    assert_eq!(client.pull_comments(&acme(), 1, false).unwrap(), first);
    assert_eq!(github.not_modified(), 3);

    // A fresh read sends no ETag.
    assert_eq!(client.pull_comments(&acme(), 1, true).unwrap(), first);
    assert_eq!(github.not_modified(), 3);

    github.edit_comment(id, "Why not?");
    let edited = client.pull_comments(&acme(), 1, false).unwrap();
    assert_eq!(edited[0].body, "Why not?");
    assert_eq!(github.not_modified(), 5);
}

#[test]
fn follows_the_pages_of_a_long_list() {
    let (github, client, _repo) = with_pull();
    github.page_size(2);
    for n in 0..5 {
        github.comment(
            "acme/app",
            1,
            GithubNewComment::issue("ana", &format!("{n}")),
        );
    }
    let bodies = |comments: Vec<PullComment>| -> Vec<String> {
        comments.into_iter().map(|c| c.body).collect()
    };
    let all = ["0", "1", "2", "3", "4"];
    assert_eq!(
        bodies(client.pull_comments(&acme(), 1, false).unwrap()),
        all
    );
    // A later page's change wouldn't show in the first's ETag, so a list
    // of pages is read whole every time.
    github.edit_comment(5, "Last");
    let again = bodies(client.pull_comments(&acme(), 1, false).unwrap());
    assert_eq!(again, ["0", "1", "2", "3", "Last"]);
    assert_eq!(github.not_modified(), 2);
}

#[test]
fn reads_one_comment_a_list_left_out_and_none_once_deleted() {
    let (github, client, _repo) = with_pull();
    let inline = GithubNewComment::inline("codex[bot]", "Leaks", "login.rs", None, 1);
    let ids = [
        github.comment("acme/app", 1, inline),
        github.comment("acme/app", 1, GithubNewComment::issue("ana", "Why?")),
        github.comment("acme/app", 1, GithubNewComment::review("ana", "Close")),
    ];
    let listed = client.pull_comments(&acme(), 1, true).unwrap();
    for id in ids {
        github.unlist_comment(id);
    }
    assert_eq!(client.pull_comments(&acme(), 1, true).unwrap(), []);
    for comment in &listed {
        let one = client.pull_comment(&acme(), 1, comment.kind, comment.id);
        assert_eq!(one.unwrap().as_ref(), Some(comment));
    }

    github.delete_comment(ids[1]);
    let gone = client.pull_comment(&acme(), 1, PullCommentKind::IssueComment, ids[1]);
    assert_eq!(gone.unwrap(), None);
    // A review asked for on another pull request isn't there.
    let elsewhere = client.pull_comment(&acme(), 2, PullCommentKind::Review, ids[2]);
    assert_eq!(elsewhere.unwrap(), None);

    github.log_out();
    let err = client
        .pull_comment(&acme(), 1, PullCommentKind::ReviewComment, ids[0])
        .unwrap_err();
    assert!(matches!(err, GithubError::NotLoggedIn(_)), "{err:?}");
}

#[test]
fn comments_on_a_missing_pull_request_are_an_error() {
    let (_github, client, _repo) = setup();
    let err = client.pull_comments(&acme(), 9, false).unwrap_err();
    assert!(matches!(err, GithubError::Failed(_)), "{err:?}");
}
