use super::*;
use crate::agent::Agent;
use crate::fake::{FakeGit, FakeStore, commit};
use crate::feedback::{Notice, Source};
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

fn notify(store: &mut FakeStore, revision: u32, state: ItemState, kind: NoticeKind) {
    let notice = Notice {
        revision,
        source: Source::Operator,
        kind,
    };
    store.notify(&id(), state, notice).unwrap();
}

fn conflict() -> NoticeKind {
    NoticeKind::MergeConflict {
        target_commit: commit(7),
        paths: vec!["README.md".into()],
    }
}

/// `app#2`, which `app#1` is stacked on, moved to `onto`, or merged without
/// `app#1`.
fn parent(onto: u8, merged: bool) -> NoticeKind {
    NoticeKind::ParentUpdated {
        parent: ItemId::parse("app#2").unwrap(),
        merged,
        onto: commit(onto),
        upstream: commit(1),
    }
}

/// The notice `mg resume` would have the agent rebase by.
fn pending(store: &FakeStore) -> Option<NoticeKind> {
    let found = resume(store, &id()).unwrap().unwrap();
    found.pending.map(|event| match event.kind {
        EventKind::Notice(notice) => notice.kind,
        other => panic!("not a notice: {other:?}"),
    })
}

#[test]
fn holds_the_conflict_its_newest_revision_has_yet_to_answer() {
    let mut store = FakeStore::with_item(2);
    notify(&mut store, 1, ItemState::ChangesRequested, conflict());
    assert_eq!(pending(&store), None, "r2 answered r1's");
    notify(&mut store, 2, ItemState::ChangesRequested, conflict());
    assert_eq!(pending(&store), Some(conflict()));
    store.set_state(&id(), ItemState::Abandoned);
    assert_eq!(pending(&store), None, "a closed item asks for none");
}

#[test]
fn a_parents_newer_notice_replaces_the_rebase_its_older_one_asked_for() {
    let mut store = FakeStore::with_item(1);
    notify(&mut store, 1, ItemState::NeedsReview, parent(8, false));
    notify(&mut store, 1, ItemState::NeedsReview, parent(9, true));
    assert_eq!(pending(&store), Some(parent(9, true)));
}

#[test]
fn an_approved_item_rebases_only_once_its_parent_merged_without_it() {
    let mut store = FakeStore::with_item(1);
    notify(&mut store, 1, ItemState::Merging, parent(8, false));
    assert_eq!(pending(&store), None, "it lands with its parent");
    notify(&mut store, 1, ItemState::Merging, parent(9, true));
    assert_eq!(pending(&store), Some(parent(9, true)));
}
