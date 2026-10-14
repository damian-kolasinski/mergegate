use super::*;
use crate::fake::{FakeGit, FakeStore};
use crate::item::ItemState;
use crate::review::{Side, SubmitReview, Verdict, WriteDraft, submit, write_draft};

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

/// `app#1` at r1, where review 1 sent `c_1` and `c_2` is still a draft.
fn reviewed() -> FakeStore {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let draft = || WriteDraft {
        id: id("app#1"),
        revision: 1,
        path: "README.md".into(),
        side: Side::New,
        start_line: 1,
        line: 1,
        body: "Why?".into(),
    };
    write_draft(&mut store, &git, draft()).unwrap();
    let review = SubmitReview {
        id: id("app#1"),
        revision: 1,
        verdict: Verdict::ChangesRequested,
        summary: String::new(),
    };
    submit(&mut store, review).unwrap();
    write_draft(&mut store, &git, draft()).unwrap();
    store
}

fn on(comment: u64, status: ReplyStatus, body: &str) -> WriteReply {
    WriteReply {
        comment: CommentId(comment),
        status,
        body: body.into(),
    }
}

#[test]
fn the_agent_answers_a_sent_comment() {
    let mut store = reviewed();
    let addressed = reply(&mut store, on(1, ReplyStatus::Addressed, "")).unwrap();
    assert_eq!(addressed.comment, CommentId(1));
    assert_eq!(addressed.item, id("app#1"));
    assert_eq!(addressed.author, Author::Agent);
    assert_eq!(addressed.status, Some(ReplyStatus::Addressed));
    assert_eq!(addressed.revision, 1);
    assert_eq!(addressed.review, None);

    let rejected = reply(&mut store, on(1, ReplyStatus::Rejected, " Out of scope.\n")).unwrap();
    assert_eq!(rejected.body, "Out of scope.");
    assert_eq!(store.replies(&id("app#1")).unwrap(), [addressed, rejected]);
}

#[test]
fn a_draft_is_no_comment_to_the_agent() {
    let mut store = reviewed();
    for comment in [2, 9] {
        let err = reply(&mut store, on(comment, ReplyStatus::Addressed, "")).unwrap_err();
        assert!(matches!(err, ReplyError::NoComment(c) if c == CommentId(comment)));
        assert_eq!(err.to_string(), format!("no comment c_{comment}"));
    }
    assert_eq!(store.replies(&id("app#1")).unwrap(), []);
}

#[test]
fn a_rejection_or_a_question_says_why() {
    let mut store = reviewed();
    for status in [ReplyStatus::Rejected, ReplyStatus::Question] {
        let err = reply(&mut store, on(1, status, " \n")).unwrap_err();
        assert!(matches!(err, ReplyError::NeedsBody(s) if s == status));
    }
    let err = reply(&mut store, on(1, ReplyStatus::Rejected, "")).unwrap_err();
    assert_eq!(err.to_string(), "a rejected reply needs a body saying why");
}

#[test]
fn a_closed_item_takes_no_reply() {
    let mut store = reviewed();
    store.set_state(&id("app#1"), ItemState::Merging);
    let err = reply(&mut store, on(1, ReplyStatus::Addressed, "")).unwrap_err();
    assert!(matches!(err, ReplyError::Review(ReviewError::Merging(_))));
    store.set_state(&id("app#1"), ItemState::Merged);
    let err = reply(&mut store, on(1, ReplyStatus::Addressed, "")).unwrap_err();
    assert!(matches!(err, ReplyError::Review(ReviewError::Closed(_))));
}

#[test]
fn an_addressed_reply_is_addressed_in_the_next_revision() {
    let addressed = Reply {
        id: 1,
        comment: CommentId(1),
        item: id("app#1"),
        author: Author::Agent,
        status: Some(ReplyStatus::Addressed),
        body: String::new(),
        revision: 2,
        review: None,
        overrides: false,
        created_at: String::new(),
    };
    assert_eq!(addressed.addressed_in(2), None);
    assert_eq!(addressed.addressed_in(3), Some(3));
    assert_eq!(addressed.addressed_in(5), Some(3));
    let rejected = Reply {
        status: Some(ReplyStatus::Rejected),
        ..addressed
    };
    assert_eq!(rejected.addressed_in(3), None);
}
