use super::*;
use crate::external::External;
use crate::github::PullCommentKind;
use crate::item::ItemId;
use crate::review::CommentId;

fn comment(id: u64, external: Option<External>) -> Comment {
    Comment {
        id: CommentId(id),
        item: ItemId::parse("app#1").unwrap(),
        revision: 1,
        anchor: None,
        body: "why?".into(),
        review: external.is_none().then_some(1),
        external,
        created_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn from_github(agreed_reply: Option<u64>) -> Option<External> {
    Some(External {
        kind: PullCommentKind::Review,
        github_id: 7,
        login: "codex[bot]".into(),
        bot: true,
        url: "https://github.com/acme/app/pull/1#discussion_r7".into(),
        path: None,
        pending: false,
        agreed_reply,
        vote: None,
    })
}

fn reply(id: u64, comment: u64, author: Author, status: Option<ReplyStatus>) -> Reply {
    Reply {
        id,
        comment: CommentId(comment),
        item: ItemId::parse("app#1").unwrap(),
        author,
        status,
        body: String::new(),
        revision: 1,
        review: (author == Author::Operator).then_some(2),
        overrides: false,
        created_at: "2026-01-01T00:00:00Z".into(),
    }
}

const QUESTION: Option<ReplyStatus> = Some(ReplyStatus::Question);
const REJECTED: Option<ReplyStatus> = Some(ReplyStatus::Rejected);

#[test]
fn counts_threads_the_agent_left_with_a_question() {
    let comments = [comment(1, None), comment(2, None), comment(3, None)];
    let replies = [
        reply(1, 1, Author::Agent, QUESTION),
        reply(2, 2, Author::Agent, QUESTION),
        reply(3, 2, Author::Operator, None),
        reply(4, 3, Author::Agent, Some(ReplyStatus::Addressed)),
    ];
    let waiting = waiting(&comments, &replies);
    assert_eq!(waiting.questions, 1);
    assert_eq!(waiting.rejections, 0);
    assert!(waiting.any());
}

#[test]
fn a_draft_answer_still_leaves_the_agent_waiting() {
    let comments = [comment(1, None)];
    let mut answer = reply(2, 1, Author::Operator, None);
    answer.review = None;
    let replies = [reply(1, 1, Author::Agent, QUESTION), answer];
    assert_eq!(waiting(&comments, &replies).questions, 1);
}

#[test]
fn counts_rejected_github_comments_until_agreed() {
    let comments = [
        comment(1, from_github(None)),
        comment(2, from_github(Some(2))),
        comment(3, None),
    ];
    let replies = [
        reply(1, 1, Author::Agent, REJECTED),
        reply(2, 2, Author::Agent, REJECTED),
        reply(3, 3, Author::Agent, REJECTED),
    ];
    let waiting = waiting(&comments, &replies);
    assert_eq!(waiting.rejections, 1);
    assert_eq!(waiting.questions, 0);
}
