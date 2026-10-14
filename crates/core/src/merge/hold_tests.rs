use super::*;
use crate::fake::{FakeBackend, FakeGit, FakeStore, commit};
use crate::feedback::EventKind;
use crate::git::GitError;
use crate::item::{Submit, submit};
use crate::merge::{Approve, Landing, approve};
use crate::project::Backend;

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn held() -> Result<Landing, GitError> {
    Err(GitError::Failed("the checkout of main has changes".into()))
}

/// `app#1` held in Merging after approving r1.
fn held_item(backend: &FakeBackend) -> FakeStore {
    let mut store = FakeStore::with_item(1);
    let request = Approve {
        id: id("app#1"),
        revision: 1,
        summary: String::new(),
    };
    approve(&mut store, &FakeGit::default(), backend, request).unwrap();
    store
}

fn last_notice(store: &FakeStore) -> NoticeKind {
    match store.events(&id("app#1"), 0).unwrap().pop().unwrap().kind {
        EventKind::Notice(notice) => notice.kind,
        kind => panic!("{kind:?}"),
    }
}

#[test]
fn retry_lands_a_held_merge() {
    let backend = FakeBackend::new(Backend::Local).then(held());
    let mut store = held_item(&backend);
    let merge = retry(&mut store, &FakeGit::default(), &backend, &id("app#1")).unwrap();
    assert_eq!(merge.item.state, ItemState::Merged);
    assert_eq!(backend.landed().len(), 2);
    assert_eq!(
        last_notice(&store),
        NoticeKind::Merged { commit: commit(9) }
    );
}

#[test]
fn retry_can_hold_again() {
    let backend = FakeBackend::new(Backend::Local).then(held()).then(held());
    let mut store = held_item(&backend);
    let merge = retry(&mut store, &FakeGit::default(), &backend, &id("app#1")).unwrap();
    assert_eq!(merge.item.state, ItemState::Merging);
    assert!(matches!(last_notice(&store), NoticeKind::MergeHeld { .. }));
}

#[test]
fn cancel_puts_the_item_back_in_needs_review_and_tells_its_agent() {
    let backend = FakeBackend::new(Backend::Local).then(held());
    let mut store = held_item(&backend);
    let item = cancel(&mut store, &id("app#1")).unwrap();
    assert_eq!(item.state, ItemState::NeedsReview);
    assert_eq!(last_notice(&store), NoticeKind::MergeCancelled);
    assert_eq!(backend.landed().len(), 1);
}

#[test]
fn only_a_merging_item_retries_or_cancels() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let backend = FakeBackend::new(Backend::Local);
    let error = retry(&mut store, &git, &backend, &id("app#1")).unwrap_err();
    assert!(matches!(error, MergeError::NotMerging(_)), "{error:?}");
    let error = cancel(&mut store, &id("app#1")).unwrap_err();
    assert!(matches!(error, MergeError::NotMerging(_)), "{error:?}");

    store.set_state(&id("app#1"), ItemState::Merged);
    let error = retry(&mut store, &git, &backend, &id("app#1")).unwrap_err();
    assert!(matches!(error, MergeError::Closed(_)), "{error:?}");
    let error = cancel(&mut store, &id("app#9")).unwrap_err();
    assert!(matches!(error, MergeError::NoItem(_)), "{error:?}");
    assert!(backend.landed().is_empty());
}

#[test]
fn a_new_revision_takes_a_held_item_back_to_needs_review() {
    let backend = FakeBackend::new(Backend::Local).then(held());
    let mut store = held_item(&backend);
    let git = FakeGit::default()
        .commit(&commit(2), None, "Add login", &["src/login.rs"])
        .commit(&commit(3), Some(&commit(2)), "Fix", &["src/fix.rs"])
        .upstream("/src/app", "main", &commit(2));
    git.push("app", "login", &commit(3));
    let request = Submit {
        project: "app".into(),
        branch: "login".into(),
        head: commit(3),
        title: None,
    };
    let item = submit(&mut store, &git, request).unwrap().item;
    assert_eq!(item.state, ItemState::NeedsReview);
    assert_eq!(item.revision.number, 2);
}
