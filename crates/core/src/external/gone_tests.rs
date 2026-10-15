use super::*;
use crate::fake::FakeGithub;

fn issue(id: u64, body: &str) -> PullComment {
    PullComment {
        kind: PullCommentKind::IssueComment,
        id,
        login: "alice".into(),
        bot: false,
        url: String::new(),
        body: body.into(),
        lines: None,
        dated: 0,
    }
}

fn key(id: u64) -> GithubKey {
    (PullCommentKind::IssueComment, id)
}

#[test]
fn only_what_github_says_is_gone_is_gone() {
    let github = FakeGithub::default();
    let repo = GithubRepo::parse("o/app").unwrap();
    for comment in [
        issue(1, "Listed"),
        issue(2, "Skipped"),
        issue(3, " "),
        issue(5, "Down"),
    ] {
        github.comment("o/app", 1, comment);
    }
    for id in [2, 3, 5] {
        github.unlist(PullCommentKind::IssueComment, id);
    }
    github.fail_asking_for(PullCommentKind::IssueComment, 5);
    let held = (1..=5).map(key).collect();
    let mut comments = vec![issue(1, "Listed")];

    let gone = confirm_gone(&github, &repo, 1, &held, &mut comments).unwrap();
    // 3 was edited down to nothing and 4 deleted; 5 waits for GitHub to
    // answer.
    assert_eq!(gone, [key(3), key(4)].into());
    assert_eq!(comments, [issue(1, "Listed"), issue(2, "Skipped")]);
}

#[test]
fn nothing_missing_asks_github_nothing() {
    let github = FakeGithub::default().logged_out();
    let repo = GithubRepo::parse("o/app").unwrap();
    let mut comments = vec![issue(1, "Listed")];
    let held = [key(1)].into();
    let gone = confirm_gone(&github, &repo, 1, &held, &mut comments).unwrap();
    assert!(gone.is_empty());
}
