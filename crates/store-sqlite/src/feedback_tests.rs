use mergegate_core::item::{ItemState, ItemStore, NewItem, NewRevision};
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
    };
    for name in ["app", "web"] {
        let name = ProjectName::parse(name).unwrap();
        store
            .insert_project(&Project {
                name: name.clone(),
                backend: Backend::Local,
                target: "main".into(),
                upstream: format!("/src/{name}").into(),
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
    }
    store
        .add_revision(&id("app#1"), "Work", ItemState::NeedsReview, revision())
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
fn cursors_are_hub_wide_and_events_per_item() {
    let (_dir, mut store) = store();
    let (app, web) = (id("app#1"), id("web#1"));
    for (item, summary) in [("app#1", "a1"), ("web#1", "w1"), ("app#1", "a2")] {
        store
            .insert_review(review(item, 1, summary), ItemState::ChangesRequested)
            .unwrap();
    }
    let cursors = |item| -> Vec<u64> {
        store
            .events(item, 0)
            .unwrap()
            .iter()
            .map(|e| e.cursor)
            .collect()
    };
    assert_eq!(cursors(&app), [1, 3]);
    assert_eq!(cursors(&web), [2]);
    assert_eq!(store.events(&app, 1).unwrap().len(), 1);
    assert_eq!(store.last_cursor(&web).unwrap(), 2);
    assert_eq!(store.last_cursor(&id("app#9")).unwrap(), 0);
}

#[test]
fn keeps_a_delivered_cursor_per_item() {
    let (_dir, mut store) = store();
    let (app, web) = (id("app#1"), id("web#1"));
    assert_eq!(store.delivered(&app).unwrap(), 0);
    store.set_delivered(&app, 3).unwrap();
    assert_eq!(store.delivered(&app).unwrap(), 3);
    assert_eq!(store.delivered(&web).unwrap(), 0);
    assert_eq!(store.delivered(&id("app#9")).unwrap(), 0);
}
