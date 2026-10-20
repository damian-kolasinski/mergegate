use super::*;
use crate::agent::Agent;
use crate::fake::{FakeGit, FakeStore};
use crate::review::{
    CommentId, ReplyStatus, Side, SubmitReview, Verdict, WriteDraft, WriteReply, reply, submit,
    write_draft,
};

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn review(store: &mut FakeStore, summary: &str, comments: &[&str]) {
    for body in comments {
        let draft = WriteDraft {
            id: id(),
            revision: 1,
            path: "README.md".into(),
            side: Side::New,
            start_line: 1,
            line: 1,
            body: (*body).into(),
        };
        write_draft(store, &FakeGit::default(), draft).unwrap();
    }
    let review = SubmitReview {
        id: id(),
        revision: 1,
        verdict: Verdict::ChangesRequested,
        summary: summary.into(),
    };
    submit(store, &FakeGit::default(), review).unwrap();
}

#[test]
fn holds_the_newest_review_and_the_open_comments() {
    let mut store = FakeStore::with_item(1);
    let agent = Agent {
        worktree: Some("/work/login".into()),
        wake: Some("harness resume".into()),
        device: None,
    };
    store.set_agent(&id(), &agent).unwrap();
    review(&mut store, "First pass", &["Rename it", "Why?"]);
    review(&mut store, "Still racy", &[]);
    let rejected = WriteReply {
        comment: CommentId(1),
        status: ReplyStatus::Rejected,
        body: "It's the public name.".into(),
    };
    reply(&mut store, rejected).unwrap();

    let found = resume(&store, &id()).unwrap().unwrap();
    assert_eq!(found.item.id, id());
    assert_eq!(found.worktree.as_deref(), Some("/work/login"));
    assert_eq!(found.wake.as_deref(), Some("harness resume"));
    assert_eq!(found.review.unwrap().summary, "Still racy");
    let open: Vec<_> = found.open.iter().map(|t| t.comment.body.as_str()).collect();
    assert_eq!(open, ["Why?"]);
}

#[test]
fn reads_nothing_it_would_acknowledge() {
    let mut store = FakeStore::with_item(1);
    review(&mut store, "", &["Why?"]);
    let found = resume(&store, &id()).unwrap().unwrap();
    assert_eq!(found.worktree, None);
    assert_eq!(found.open.len(), 1);
    assert_eq!(store.delivered(&id()).unwrap(), 0);
}

#[test]
fn an_unknown_item_has_none() {
    let store = FakeStore::with_item(1);
    let other = ItemId::parse("app#2").unwrap();
    assert_eq!(resume(&store, &other).unwrap(), None);
}
