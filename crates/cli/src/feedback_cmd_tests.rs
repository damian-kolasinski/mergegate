use api::review::{Author, Reply};

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
        EventBody::ItemAbandoned { reason: None },
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
        },
    ]
}

#[test]
fn names_each_event_by_its_json_type() {
    for body in bodies() {
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(kind(&body), json["type"], "{json}");
    }
}

#[test]
fn heads_an_event_with_its_type() {
    let merged = Event {
        cursor: 9,
        item: "app#1".into(),
        revision: 2,
        source: Source::Operator,
        body: EventBody::ItemMerged {
            commit: "abc".into(),
            branch: "feature".into(),
            commands: vec!["git fetch mergegate".into()],
        },
    };
    assert_eq!(
        event(&merged),
        "[9] item.merged app#1 r2: the operator merged it as abc\n\n  \
         Clean up feature as agreed with the operator; by default:\n    \
         git fetch mergegate"
    );
}
