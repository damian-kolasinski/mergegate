use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};
use mergegate_core::review::{NewReview, ReviewStore, Verdict};

use super::*;

/// `app#1` with two revisions and `web#1` with one.
fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    let revision = || NewRevision {
        head: "a".repeat(40),
        base: "0".repeat(40),
        files: Vec::new(),
        guide: None,
        interdiffs: Vec::new(),
        viewed: Vec::new(),
        parent: None,
    };
    for name in ["app", "web"] {
        let name = ProjectName::parse(name).unwrap();
        store
            .insert_project(&Project {
                name: name.clone(),
                backend: Backend::Local,
                target: "main".into(),
                upstream: format!("/src/{name}").into(),
                github: None,
                bots: Vec::new(),
            })
            .unwrap();
        store
            .insert_item(NewItem {
                project: name,
                branch: "work".into(),
                title: "Work".into(),
                state: ItemState::NeedsReview,
                backend: ItemBackend::Local,
                target: "main".into(),
                revision: revision(),
                priority: None,
                note: None,
            })
            .unwrap();
    }
    store
        .add_revision(&id("app#1"), "Work", ItemState::NeedsReview, revision())
        .unwrap();
    (dir, store)
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn log(turn: u32, revision: u32, ms: u64) -> Logged {
    Logged { turn, revision, ms }
}

#[test]
fn adds_to_each_turns_time_on_each_revision() {
    let (_dir, mut store) = store();
    store
        .add_review_time(&id("app#1"), log(1, 1, 30_000))
        .unwrap();
    store
        .add_review_time(&id("app#1"), log(1, 1, 12_000))
        .unwrap();
    store
        .add_review_time(&id("app#1"), log(2, 1, 1_000))
        .unwrap();
    store
        .add_review_time(&id("app#1"), log(2, 2, 5_000))
        .unwrap();

    let time = store.review_time(&id("app#1")).unwrap();

    assert_eq!(
        time,
        [log(1, 1, 42_000), log(2, 1, 1_000), log(2, 2, 5_000)]
    );
}

#[test]
fn keeps_items_apart() {
    let (_dir, mut store) = store();
    store
        .add_review_time(&id("app#1"), log(1, 1, 30_000))
        .unwrap();
    let review = NewReview {
        item: id("app#1"),
        revision: 1,
        verdict: Verdict::Comment,
        summary: String::new(),
        moved: Vec::new(),
    };
    store.insert_review(review, ItemState::NeedsReview).unwrap();

    assert!(store.review_time(&id("web#1")).unwrap().is_empty());
    assert!(store.sent_reviews(&id("web#1")).unwrap().is_empty());
}

#[test]
fn refuses_a_revision_the_item_lacks() {
    let (_dir, mut store) = store();

    assert!(
        store
            .add_review_time(&id("web#1"), log(1, 2, 1_000))
            .is_err()
    );
}

#[test]
fn lists_the_reviews_sent_oldest_first() {
    let (_dir, mut store) = store();
    for (revision, verdict) in [(1, Verdict::ChangesRequested), (2, Verdict::Approved)] {
        let review = NewReview {
            item: id("app#1"),
            revision,
            verdict,
            summary: "Summary".into(),
            moved: Vec::new(),
        };
        store.insert_review(review, ItemState::NeedsReview).unwrap();
    }

    let sent = store.sent_reviews(&id("app#1")).unwrap();

    let got: Vec<_> = sent.iter().map(|r| (r.revision, r.verdict)).collect();
    assert_eq!(
        got,
        [(1, Verdict::ChangesRequested), (2, Verdict::Approved)]
    );
    assert!(sent.iter().all(|r| !r.submitted_at.is_empty()));
}
