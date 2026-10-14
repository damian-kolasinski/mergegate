use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};

use super::*;

/// `app#1` with two revisions and `web#1` with one, all changing nothing:
/// the store doesn't check paths, the domain does.
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
            })
            .unwrap();
    }
    let app = id("app#1");
    store
        .add_revision(&app, "Work", ItemState::NeedsReview, revision())
        .unwrap();
    (dir, store)
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn viewed(store: &Store, item: &str, revision: u32) -> Vec<String> {
    store
        .viewed(&id(item), revision)
        .unwrap()
        .into_iter()
        .collect()
}

#[test]
fn keeps_marks_per_item_and_revision() {
    let (_dir, mut store) = store();
    let app = id("app#1");
    store.set_viewed(&app, 2, "b.rs", true).unwrap();
    store.set_viewed(&app, 2, "a.rs", true).unwrap();
    store.set_viewed(&app, 2, "a.rs", true).unwrap();
    store.set_viewed(&app, 1, "c.rs", true).unwrap();

    assert_eq!(viewed(&store, "app#1", 2), ["a.rs", "b.rs"]);
    assert_eq!(viewed(&store, "app#1", 1), ["c.rs"]);
    assert_eq!(viewed(&store, "web#1", 1), Vec::<String>::new());
}

#[test]
fn unmarking_removes_only_that_mark() {
    let (_dir, mut store) = store();
    let app = id("app#1");
    store.set_viewed(&app, 2, "a.rs", true).unwrap();
    store.set_viewed(&app, 2, "b.rs", true).unwrap();
    store.set_viewed(&id("web#1"), 1, "a.rs", true).unwrap();

    store.set_viewed(&app, 2, "a.rs", false).unwrap();
    store.set_viewed(&app, 2, "a.rs", false).unwrap();
    assert_eq!(viewed(&store, "app#1", 2), ["b.rs"]);
    assert_eq!(viewed(&store, "web#1", 1), ["a.rs"]);
}

#[test]
fn a_mark_needs_its_revision() {
    let (_dir, mut store) = store();
    store.set_viewed(&id("web#1"), 2, "a.rs", true).unwrap_err();
    assert_eq!(viewed(&store, "web#1", 2), Vec::<String>::new());
}
