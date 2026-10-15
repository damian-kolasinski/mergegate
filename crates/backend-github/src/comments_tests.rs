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
        "created_at": "2026-01-02T03:04:05Z",
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
            created: 1_767_323_045_000,
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

#[test]
fn reads_github_s_times_to_the_millisecond() {
    for (stamp, expected) in [
        ("1970-01-01T00:00:00Z", Some(0)),
        ("2026-01-02T03:04:05Z", Some(1_767_323_045_000)),
        ("2024-02-29T23:59:59.25Z", Some(1_709_251_199_250)),
        ("2000-03-01T00:00:00Z", Some(951_868_800_000)),
        ("2026-01-02T03:04:05+00:00", None),
        ("2026-13-02T03:04:05Z", None),
        ("2026-1-02T03:04:05Z", None),
        ("2026-01-02 03:04:05Z", None),
        ("1969-12-31T23:59:59Z", None),
        ("", None),
    ] {
        assert_eq!(millis(stamp), expected, "{stamp}");
    }
}

#[test]
fn a_review_is_dated_by_its_submission_and_an_undated_comment_by_the_epoch() {
    let review = json!({
        "id": 9,
        "user": { "login": "ana", "type": "User" },
        "html_url": "https://github.com/acme/app/pull/1#pullrequestreview-9",
        "body": "Close",
        "state": "COMMENTED",
        "submitted_at": "2026-01-02T03:04:05Z",
    });
    let review = comment(PullCommentKind::Review, &review).unwrap().unwrap();
    assert_eq!(review.created, 1_767_323_045_000);
    let issue = json!({ "id": 1, "html_url": "x", "body": "Hi", "user": { "login": "ana" } });
    let issue = comment(PullCommentKind::IssueComment, &issue)
        .unwrap()
        .unwrap();
    assert_eq!(issue.created, 0);
}
