use mergegate_core::feedback::{EventKind, FeedbackStore};
use mergegate_core::item::{ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};
use mergegate_core::review::{NewReview, Verdict};

use super::*;

/// `app#1` with two revisions.
fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    let revision = || NewRevision {
        head: "a".repeat(40),
        base: "0".repeat(40),
        files: Vec::new(),
    };
    let name = ProjectName::parse("app").unwrap();
    store
        .insert_project(&Project {
            name: name.clone(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
        })
        .unwrap();
    store
        .insert_item(NewItem {
            project: name,
            branch: "work".into(),
            title: "Work".into(),
            state: ItemState::NeedsReview,
            backend: Backend::Local,
            target: "main".into(),
            revision: revision(),
        })
        .unwrap();
    let app = ItemId::parse("app#1").unwrap();
    store
        .add_revision(&app, "Work", ItemState::NeedsReview, revision())
        .unwrap();
    (dir, store)
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn review(item: &str, revision: u32, summary: &str) -> NewReview {
    NewReview {
        item: id(item),
        revision,
        verdict: Verdict::ChangesRequested,
        summary: summary.into(),
    }
}

#[test]
fn a_review_moves_the_item_and_logs_one_event() {
    let (_dir, mut store) = store();
    let app = id("app#1");
    let written = store
        .insert_review(review("app#1", 1, "Fix it"), ItemState::ChangesRequested)
        .unwrap();
    assert_eq!(written.revision, 1);
    assert_eq!(written.summary, "Fix it");
    assert!(written.submitted_at.ends_with('Z'));
    let item = store.item(&app).unwrap().unwrap();
    assert_eq!(item.state, ItemState::ChangesRequested);
    // The review doesn't add a revision.
    assert_eq!(item.revision.number, 2);

    let events = store.events(&app, 0).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::ReviewSubmitted(written));
    assert_eq!(store.last_cursor(&app).unwrap(), events[0].cursor);
}
