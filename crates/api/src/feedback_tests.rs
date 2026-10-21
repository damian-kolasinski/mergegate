use crate::review::{Author, Comment, Reply, Verdict};
use crate::review_time::{RevisionStats, TurnStats};

use super::*;

fn comment() -> Comment {
    Comment {
        id: "c_1".into(),
        revision: 1,
        path: None,
        side: None,
        start_line: None,
        line: None,
        body: "Rename it.".into(),
        quote: None,
        external: None,
        placement: None,
    }
}

fn bodies() -> Vec<EventBody> {
    vec![
        EventBody::ReviewSubmitted {
            verdict: Verdict::Approved,
            summary: String::new(),
            comments: vec![],
            replies: vec![],
        },
        EventBody::ItemAbandoned {
            reason: None,
            stats: None,
        },
        EventBody::MergeHeld {
            reason: String::new(),
        },
        EventBody::MergeCancelled {},
        EventBody::MergeConflict {
            target_commit: String::new(),
            paths: vec![],
            commands: vec![],
        },
        EventBody::ExternalComments {
            comments: vec![],
            edited: vec![],
        },
        EventBody::CommentOverride {
            comment: Box::new(comment()),
            reply: Reply {
                id: 1,
                comment: "c_1".into(),
                author: Author::Operator,
                status: None,
                body: String::new(),
                addressed_in: None,
                overrides: true,
                created_at: String::new(),
            },
        },
        EventBody::ItemMerged {
            commit: String::new(),
            branch: String::new(),
            commands: vec![],
            stats: None,
        },
    ]
}

#[test]
fn each_kind_is_the_type_it_serializes_under() {
    for body in bodies() {
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(body.kind(), json["type"], "{json}");
    }
}

#[test]
fn reads_a_merge_from_a_hub_that_sends_no_commands() {
    let json = r#"{"type":"item.merged","commit":"abc","branch":"feature"}"#;
    let body: EventBody = serde_json::from_str(json).unwrap();
    assert_eq!(
        body,
        EventBody::ItemMerged {
            commit: "abc".into(),
            branch: "feature".into(),
            commands: vec![],
            stats: None,
        }
    );
}

#[test]
fn carries_a_closes_stats_for_telemetry() {
    let body = EventBody::ItemMerged {
        commit: "abc".into(),
        branch: "feature".into(),
        commands: vec![],
        stats: Some(ReviewStats {
            opened_at: "2026-10-20T09:00:00.000Z".into(),
            closed_at: Some("2026-10-20T11:00:00.000Z".into()),
            review_ms: 90_000,
            turns: vec![TurnStats {
                turn: 1,
                revision: 1,
                verdict: Some(Verdict::Approved),
                sent_at: Some("2026-10-20T10:59:00.000Z".into()),
                review_ms: 90_000,
            }],
            revisions: vec![RevisionStats {
                revision: 1,
                submitted_at: "2026-10-20T09:00:00.000Z".into(),
                review_ms: 90_000,
            }],
        }),
    };

    assert_eq!(
        serde_json::to_value(&body).unwrap()["stats"],
        serde_json::json!({
            "opened_at": "2026-10-20T09:00:00.000Z",
            "closed_at": "2026-10-20T11:00:00.000Z",
            "review_ms": 90_000,
            "turns": [{
                "turn": 1,
                "revision": 1,
                "verdict": "approved",
                "sent_at": "2026-10-20T10:59:00.000Z",
                "review_ms": 90_000,
            }],
            "revisions": [{
                "revision": 1,
                "submitted_at": "2026-10-20T09:00:00.000Z",
                "review_ms": 90_000,
            }],
        })
    );
}
