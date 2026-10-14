use super::*;
use crate::fake::{FakeBackend, FakeGit, FakeStore, commit};
use crate::git::GitError;
use crate::item::{ItemId, ItemStore};
use crate::merge::{Approve, Landing, approve, cancel};
use crate::project::Backend;
use crate::review::{NewReview, ReviewStore, SubmitReview, Verdict, submit};

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn approved(landing: Result<Landing, GitError>) -> FakeStore {
    let mut store = FakeStore::with_item(1);
    let backend = FakeBackend::new(Backend::Local).then(landing);
    let request = Approve {
        id: id("app#1"),
        revision: 1,
        summary: String::new(),
    };
    approve(&mut store, &FakeGit::default(), &backend, request).unwrap();
    store
}

fn blocked_now(store: &FakeStore) -> Option<Blocked> {
    let item = store.item(&id("app#1")).unwrap().unwrap();
    blocked(store, &item).unwrap()
}

fn conflict() -> Landing {
    Landing::Conflict {
        target_commit: commit(7),
        paths: vec!["README.md".into()],
    }
}

#[test]
fn an_item_without_a_merge_is_not_blocked() {
    assert_eq!(blocked_now(&FakeStore::with_item(1)), None);
}

#[test]
fn a_held_merge_blocks_until_cancelled() {
    let mut store = approved(Err(GitError::Failed("dirty".into())));
    let held = Blocked::Held {
        reason: "dirty".into(),
    };
    assert_eq!(blocked_now(&store), Some(held));

    cancel(&mut store, &id("app#1")).unwrap();
    assert_eq!(blocked_now(&store), None);
}

#[test]
fn a_conflict_blocks_through_later_comments_but_not_a_later_approval() {
    let mut store = approved(Ok(conflict()));
    let expected = Blocked::Conflict {
        target_commit: commit(7),
        paths: vec!["README.md".into()],
    };
    assert_eq!(blocked_now(&store), Some(expected.clone()));

    let comment = SubmitReview {
        id: id("app#1"),
        revision: 1,
        verdict: Verdict::Comment,
        summary: "Rebase onto main.".into(),
    };
    submit(&mut store, comment).unwrap();
    assert_eq!(blocked_now(&store), Some(expected));

    // Approving the same revision again, which still conflicts on the
    // backend's side, is a new merge the last notice speaks for.
    let backend = FakeBackend::new(Backend::Local);
    let request = Approve {
        id: id("app#1"),
        revision: 1,
        summary: String::new(),
    };
    approve(&mut store, &FakeGit::default(), &backend, request).unwrap();
    assert_eq!(blocked_now(&store), None);
}

#[test]
fn a_merged_item_is_not_blocked() {
    let store = approved(Ok(Landing::Merged { commit: commit(9) }));
    assert_eq!(blocked_now(&store), None);
}

#[test]
fn a_merge_cut_short_before_it_logged_anything_is_held() {
    // What a hub that stopped mid-land leaves: the approval, and no outcome.
    let mut store = FakeStore::with_item(1);
    let review = NewReview {
        item: id("app#1"),
        revision: 1,
        verdict: Verdict::Approved,
        summary: String::new(),
    };
    store.insert_review(review, ItemState::Merging).unwrap();
    assert!(matches!(blocked_now(&store), Some(Blocked::Held { .. })));
}
