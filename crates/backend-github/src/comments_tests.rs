use serde_json::json;

use super::*;

#[test]
fn reads_an_inline_comment_on_the_old_side() {
    let value = json!({
        "id": 7,
        "user": { "login": "codex[bot]", "type": "Bot" },
        "html_url": "https://github.com/acme/app/pull/1#discussion_r7",
        "body": "Leaks",
        "path": "src/login.rs",
        "side": "LEFT",
        "original_commit_id": "a",
        "original_start_line": null,
        "original_line": 3,
        "commit_id": "b",
        "start_line": null,
        "line": null,
    });
    let comment = comment(PullCommentKind::ReviewComment, &value)
        .unwrap()
        .unwrap();
    assert_eq!(
        comment,
        PullComment {
            kind: PullCommentKind::ReviewComment,
            id: 7,
            login: "codex[bot]".into(),
            bot: true,
            url: "https://github.com/acme/app/pull/1#discussion_r7".into(),
            body: "Leaks".into(),
            lines: Some(PullLines {
                path: "src/login.rs".into(),
                old_side: true,
                original_commit: "a".into(),
                original_start_line: None,
                original_line: Some(3),
                commit: "b".into(),
                start_line: None,
                line: None,
            }),
        }
    );
}

#[test]
fn skips_a_pending_review_and_reads_a_bodiless_one_as_empty() {
    let review = |state| {
        json!({
            "id": 9,
            "user": null,
            "html_url": "https://github.com/acme/app/pull/1#pullrequestreview-9",
            "body": null,
            "state": state,
        })
    };
    let kind = PullCommentKind::Review;
    assert_eq!(comment(kind, &review("PENDING")).unwrap(), None);
    let approved = comment(kind, &review("APPROVED")).unwrap().unwrap();
    assert_eq!(
        (approved.body.as_str(), approved.login.as_str()),
        ("", "ghost")
    );
    assert!(!approved.bot);
}

#[test]
fn a_comment_without_an_id_is_an_error() {
    let value = json!({ "html_url": "x", "body": "Hi", "user": { "login": "ana" } });
    let err = comment(PullCommentKind::IssueComment, &value).unwrap_err();
    assert_eq!(err.to_string(), "GitHub sent a comment without id");
}
