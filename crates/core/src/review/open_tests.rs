use super::*;
use crate::external::External;
use crate::github::PullCommentKind;
use crate::item::ItemId;
use crate::review::{Anchor, CommentId, Side};

fn comment(id: u64, review: Option<u64>) -> Comment {
    Comment {
        id: CommentId(id),
        item: ItemId::parse("app#1").unwrap(),
        revision: 1,
        anchor: Some(Anchor {
            path: "src/login.rs".into(),
            side: Side::New,
            start_line: 1,
            line: 1,
            quote: "fn login()".into(),
        }),
        body: "why?".into(),
        review,
        external: None,
        created_at: "2026-01-01T00:00:00Z".into(),
    }
}

/// `(author, status, revision written at, sent)` as a reply on c_1.
type R = (Author, Option<ReplyStatus>, u32, bool);

const AGENT: Author = Author::Agent;
const OPERATOR: Author = Author::Operator;
const ADDRESSED: Option<ReplyStatus> = Some(ReplyStatus::Addressed);
const REJECTED: Option<ReplyStatus> = Some(ReplyStatus::Rejected);
const QUESTION: Option<ReplyStatus> = Some(ReplyStatus::Question);

fn replies(replies: &[R]) -> Vec<Reply> {
    (1..)
        .zip(replies)
        .map(|(id, &(author, status, revision, sent))| Reply {
            id,
            comment: CommentId(1),
            item: ItemId::parse("app#1").unwrap(),
            author,
            status,
            body: String::new(),
            revision,
            review: (author == OPERATOR && sent).then_some(9),
            overrides: false,
            created_at: "2026-01-01T00:00:00Z".into(),
        })
        .collect()
}

#[test]
fn a_comment_is_open_until_its_agent_settles_it() {
    // (replies on c_1, newest revision, open)
    let cases: &[(&[R], u32, bool)] = &[
        (&[], 1, true),
        (&[(AGENT, QUESTION, 1, true)], 1, true),
        (&[(AGENT, REJECTED, 1, true)], 1, false),
        // Addressed, but the revision with the change isn't submitted yet.
        (&[(AGENT, ADDRESSED, 1, true)], 1, true),
        (&[(AGENT, ADDRESSED, 1, true)], 2, false),
        // The operator wrote back.
        (
            &[(AGENT, REJECTED, 1, true), (OPERATOR, None, 1, true)],
            1,
            true,
        ),
        (
            &[(AGENT, QUESTION, 1, true), (OPERATOR, None, 1, true)],
            1,
            true,
        ),
        (
            &[(AGENT, ADDRESSED, 1, true), (OPERATOR, None, 2, true)],
            2,
            true,
        ),
        // ...and the agent answered that.
        (
            &[
                (AGENT, QUESTION, 1, true),
                (OPERATOR, None, 1, true),
                (AGENT, ADDRESSED, 1, true),
            ],
            2,
            false,
        ),
        // A draft the operator hasn't sent doesn't count.
        (
            &[(AGENT, REJECTED, 1, true), (OPERATOR, None, 1, false)],
            1,
            false,
        ),
    ];
    for &(thread, newest, open) in cases {
        let conversation = Conversation {
            newest,
            comments: vec![comment(1, Some(9))],
            replies: replies(thread),
        };
        let threads = open_threads(&conversation);
        assert_eq!(!threads.is_empty(), open, "{thread:?} at r{newest}");
        if let Some(thread) = threads.first() {
            assert!(thread.replies.iter().all(|r| !r.is_draft()));
        }
    }
}

#[test]
fn drafts_are_not_threads() {
    let conversation = Conversation {
        newest: 1,
        comments: vec![comment(1, None), comment(2, Some(9))],
        replies: Vec::new(),
    };
    let ids: Vec<_> = open_threads(&conversation)
        .into_iter()
        .map(|t| t.comment.id)
        .collect();
    assert_eq!(ids, [CommentId(2)]);
}

#[test]
fn held_external_comments_are_not_threads_yet() {
    let external = |pending| {
        let mut comment = comment(1, None);
        comment.external = Some(External {
            kind: PullCommentKind::IssueComment,
            github_id: 7,
            login: "alice".into(),
            bot: false,
            url: "https://github.com/acme/app/pull/1#issuecomment-7".into(),
            path: None,
            pending,
            agreed_reply: None,
        });
        comment
    };
    for (pending, threads) in [(true, 0), (false, 1)] {
        let conversation = Conversation {
            newest: 1,
            comments: vec![external(pending)],
            replies: Vec::new(),
        };
        assert_eq!(open_threads(&conversation).len(), threads, "{pending}");
    }
}
