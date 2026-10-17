use super::*;
use crate::external::{NewExternal, Vote, release};
use crate::fake::{FakeGit, FakeStore};
use crate::feedback::Source;
use crate::feedback::{EventKind, FeedbackStore};
use crate::github::PullCommentKind;
use crate::item::{ItemId, ItemState};
use crate::review::{Side, WriteDraft, WriteReply, conversation, open_threads, reply, write_draft};

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

/// `app#1` with codex's `c_1`, carried to the agent, which answered `status`.
fn answered(status: ReplyStatus) -> FakeStore {
    let mut store = FakeStore::with_item(1);
    let new = NewExternal {
        item: id(),
        revision: 1,
        anchor: None,
        body: "Rename it.".into(),
        kind: PullCommentKind::IssueComment,
        github_id: 1,
        login: "codex[bot]".into(),
        bot: true,
        url: String::new(),
        path: None,
        dated: 0,
    };
    let comment = store.insert_external(new, 0).unwrap().id;
    store.set_vote(comment, Vote::Up, 0).unwrap();
    release(&mut store, &id()).unwrap();
    let answer = WriteReply {
        comment: CommentId(1),
        status,
        body: "The name is fine.".into(),
    };
    reply(&mut store, answer).unwrap();
    store
}

#[test]
fn agreeing_marks_the_rejection_and_sends_nothing() {
    let mut store = answered(ReplyStatus::Rejected);
    let events = store.events(&id(), 0).unwrap().len();
    let agreed = agree(&mut store, CommentId(1)).unwrap();
    assert_eq!(agreed.external.unwrap().agreed_reply, Some(1));
    let stored = store.comment(CommentId(1)).unwrap().unwrap();
    assert_eq!(stored.external.unwrap().agreed_reply, Some(1));
    assert_eq!(store.events(&id(), 0).unwrap().len(), events);
}

#[test]
fn an_override_reopens_the_thread_at_once() {
    let mut store = answered(ReplyStatus::Rejected);
    let threads = |store: &FakeStore| {
        open_threads(
            &conversation(store, &FakeGit::default(), &id(), None)
                .unwrap()
                .unwrap(),
        )
    };
    assert!(threads(&store).is_empty());

    let (reply, event) =
        override_rejection(&mut store, CommentId(1), " It clashes. ".into()).unwrap();
    assert!(reply.overrides && !reply.is_draft());
    assert_eq!(
        (reply.author, reply.body.as_str()),
        (Author::Operator, "It clashes.")
    );
    assert_eq!(event.source(), Source::Operator);
    let EventKind::Override(o) = &event.kind else {
        panic!("{event:?}");
    };
    assert_eq!((o.comment.id, o.reply.id), (CommentId(1), reply.id));
    assert_eq!(
        store.item(&id()).unwrap().unwrap().state,
        ItemState::NeedsReview
    );
    assert_eq!(threads(&store).len(), 1);

    // The operator spoke last, so there's no rejection to override now.
    let again = override_rejection(&mut store, CommentId(1), String::new());
    assert!(
        matches!(again, Err(ExternalError::NotRejected(_))),
        "{again:?}"
    );
}

#[test]
fn only_a_rejection_can_be_agreed_with_or_overridden() {
    let mut store = answered(ReplyStatus::Question);
    let agreed = agree(&mut store, CommentId(1));
    assert!(
        matches!(agreed, Err(ExternalError::NotRejected(_))),
        "{agreed:?}"
    );

    // The operator's own comments have no rejection from GitHub to settle.
    let draft = WriteDraft {
        id: id(),
        revision: 1,
        path: "README.md".into(),
        side: Side::New,
        start_line: 1,
        line: 1,
        body: "Why?".into(),
    };
    let own = write_draft(&mut store, &FakeGit::default(), draft).unwrap();
    let overridden = override_rejection(&mut store, own.id, String::new());
    assert!(
        matches!(overridden, Err(ExternalError::NoComment(_))),
        "{overridden:?}"
    );
}

#[test]
fn a_closed_item_takes_no_override() {
    let mut store = answered(ReplyStatus::Rejected);
    store.set_state(&id(), ItemState::Abandoned);
    let overridden = override_rejection(&mut store, CommentId(1), String::new());
    assert!(
        matches!(overridden, Err(ExternalError::Review(_))),
        "{overridden:?}"
    );
}
