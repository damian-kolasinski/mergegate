use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};

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

#[test]
fn adds_to_each_revisions_time() {
    let (_dir, mut store) = store();
    store.add_review_time(&id("app#1"), 1, 30_000).unwrap();
    store.add_review_time(&id("app#1"), 1, 12_000).unwrap();
    store.add_review_time(&id("app#1"), 2, 5_000).unwrap();

    let time = store.review_time(&id("app#1")).unwrap();

    assert_eq!(time, BTreeMap::from([(1, 42_000), (2, 5_000)]));
}

#[test]
fn keeps_items_apart() {
    let (_dir, mut store) = store();
    store.add_review_time(&id("app#1"), 1, 30_000).unwrap();

    assert!(store.review_time(&id("web#1")).unwrap().is_empty());
}

#[test]
fn refuses_a_revision_the_item_lacks() {
    let (_dir, mut store) = store();

    assert!(store.add_review_time(&id("web#1"), 2, 1_000).is_err());
}
