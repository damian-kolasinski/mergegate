use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectStore};

use super::*;

/// `app#1` to `app#3` and `web#1`.
fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    for (name, items) in [("app", 3), ("web", 1)] {
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
        for n in 1..=items {
            store
                .insert_item(NewItem {
                    project: name.clone(),
                    branch: format!("work-{n}"),
                    title: "Work".into(),
                    state: ItemState::NeedsReview,
                    backend: ItemBackend::Local,
                    target: "main".into(),
                    revision: NewRevision {
                        head: "a".repeat(40),
                        base: "0".repeat(40),
                        files: Vec::new(),
                        guide: None,
                        interdiffs: Vec::new(),
                        viewed: Vec::new(),
                    },
                    priority: None,
                    note: None,
                })
                .unwrap();
        }
    }
    (dir, store)
}

fn pair(item: &str, other: &str, conflicts: bool) -> Pair {
    Pair {
        item: ItemId::parse(item).unwrap(),
        item_head: "a".repeat(40),
        other: ItemId::parse(other).unwrap(),
        other_head: "b".repeat(40),
        conflicts,
    }
}

#[test]
fn replaces_a_projects_pairs_and_keeps_the_others() {
    let (_dir, mut store) = store();
    let app = ProjectName::parse("app").unwrap();
    let web = ProjectName::parse("web").unwrap();
    store
        .set_pairs(
            &app,
            &[pair("app#2", "app#3", false), pair("app#1", "app#2", true)],
        )
        .unwrap();
    store.set_pairs(&web, &[]).unwrap();
    assert_eq!(
        store.pairs(Some(&app)).unwrap(),
        [pair("app#1", "app#2", true), pair("app#2", "app#3", false)]
    );

    store
        .set_pairs(&app, &[pair("app#1", "app#3", true)])
        .unwrap();
    assert_eq!(store.pairs(None).unwrap(), [pair("app#1", "app#3", true)]);
    assert!(store.pairs(Some(&web)).unwrap().is_empty());
}
