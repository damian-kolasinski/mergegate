use super::*;
use crate::fake::{FakeGit, FakeStore};
use crate::feedback::{EventKind, FeedbackStore};
use crate::item::{ItemId, ItemState};
use crate::review::{
    ReplyStatus, Side, SubmitReview, Verdict, WriteDraft, WriteReply, reply, submit, write_draft,
};

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
    submit(&mut store, &FakeGit::default(), review("")).unwrap();
    write_draft(&mut store, &git, draft()).unwrap();
    store
}

fn review(summary: &str) -> SubmitReview {
    SubmitReview {
        id: id("app#1"),
        revision: 1,
        verdict: Verdict::Comment,
        summary: summary.into(),
    }
}

#[test]
fn the_operator_drafts_a_reply_on_a_sent_comment() {
    let mut store = reviewed();
    let draft = write_draft_reply(&mut store, CommentId(1), " Empty too.\n".into()).unwrap();
    assert_eq!(draft.comment, CommentId(1));
    assert_eq!(draft.item, id("app#1"));
    assert_eq!(draft.author, Author::Operator);
    assert_eq!(draft.status, None);
    assert_eq!(draft.body, "Empty too.");
    assert_eq!(draft.revision, 1);
    assert_eq!(draft.review, None);
    assert!(draft.is_draft());

    let edited =
        edit_draft_reply(&mut store, CommentId(1), draft.id, "Missing too.".into()).unwrap();
    assert_eq!(edited.body, "Missing too.");
    let reply = edited.id;
    assert_eq!(store.replies(&id("app#1")).unwrap(), [edited]);
    delete_draft_reply(&mut store, CommentId(1), reply).unwrap();
    assert_eq!(store.replies(&id("app#1")).unwrap(), []);
}

#[test]
fn the_next_review_sends_the_draft_replies() {
    let mut store = reviewed();
    let agent = WriteReply {
        comment: CommentId(1),
        status: ReplyStatus::Question,
        body: "Empty or missing?".into(),
    };
    let question = reply(&mut store, agent).unwrap();
    let draft = write_draft_reply(&mut store, CommentId(1), "Both.".into()).unwrap();

    // A draft reply is enough to send a review, with the draft comment too.
    let sent = submit(&mut store, &FakeGit::default(), review("")).unwrap();
    assert_eq!(sent.replies.len(), 1);
    assert_eq!(sent.replies[0].id, draft.id);
    assert_eq!(sent.replies[0].review, Some(sent.id));
    assert_eq!(sent.comments.len(), 1);
    let events = store.events(&id("app#1"), 0).unwrap();
    assert_eq!(events[1].kind, EventKind::ReviewSubmitted(sent.clone()));
    assert_eq!(
        store.replies(&id("app#1")).unwrap(),
        [question, sent.replies[0].clone()]
    );

    // Sent, it reads as the agent read it.
    let err = edit_draft_reply(&mut store, CommentId(1), draft.id, "Neither.".into()).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("reply {} was sent, so it can't change", draft.id)
    );
    let err = delete_draft_reply(&mut store, CommentId(1), draft.id).unwrap_err();
    assert!(matches!(err, DraftReplyError::Sent(r) if r == draft.id));
    assert_eq!(
        submit(&mut store, &FakeGit::default(), review(""))
            .unwrap_err()
            .to_string(),
        "a review needs a summary, a comment or a reply"
    );
}

#[test]
fn only_sent_comments_take_a_draft_reply() {
    let mut store = reviewed();
    for comment in [2, 9] {
        let err = write_draft_reply(&mut store, CommentId(comment), "Hm.".into()).unwrap_err();
        assert!(matches!(err, DraftReplyError::NoComment(c) if c == CommentId(comment)));
    }
    let err = write_draft_reply(&mut store, CommentId(1), " \n".into()).unwrap_err();
    assert_eq!(err.to_string(), "a reply needs a body");
    assert_eq!(store.replies(&id("app#1")).unwrap(), []);
}

#[test]
fn a_draft_reply_is_found_on_its_comment_only() {
    let mut store = reviewed();
    let agent = WriteReply {
        comment: CommentId(1),
        status: ReplyStatus::Addressed,
        body: String::new(),
    };
    let addressed = reply(&mut store, agent).unwrap();
    let draft = write_draft_reply(&mut store, CommentId(1), "Thanks.".into()).unwrap();
    for (comment, reply) in [
        (CommentId(2), draft.id),
        (CommentId(1), addressed.id),
        (CommentId(1), 9),
    ] {
        let err = edit_draft_reply(&mut store, comment, reply, "Hm.".into()).unwrap_err();
        assert!(
            matches!(err, DraftReplyError::NoDraft { comment: c, reply: r } if c == comment && r == reply),
            "{err}"
        );
        let err = delete_draft_reply(&mut store, comment, reply).unwrap_err();
        assert!(matches!(err, DraftReplyError::NoDraft { .. }), "{err}");
    }
    let err = edit_draft_reply(&mut store, CommentId(1), draft.id, "".into()).unwrap_err();
    assert!(matches!(err, DraftReplyError::Empty));
    let err = delete_draft_reply(&mut store, CommentId(2), 7).unwrap_err();
    assert_eq!(err.to_string(), "c_2 has no draft reply 7");
}

#[test]
fn a_closed_item_takes_no_draft_reply() {
    let mut store = reviewed();
    let draft = write_draft_reply(&mut store, CommentId(1), "Thanks.".into()).unwrap();
    store.set_state(&id("app#1"), ItemState::Merging);
    let err = write_draft_reply(&mut store, CommentId(1), "Hm.".into()).unwrap_err();
    assert!(matches!(
        err,
        DraftReplyError::Review(ReviewError::Merging(_))
    ));
    store.set_state(&id("app#1"), ItemState::Merged);
    let err = edit_draft_reply(&mut store, CommentId(1), draft.id, "Hm.".into()).unwrap_err();
    assert!(matches!(
        err,
        DraftReplyError::Review(ReviewError::Closed(_))
    ));
    // A draft that can't be sent can still go.
    delete_draft_reply(&mut store, CommentId(1), draft.id).unwrap();
}
