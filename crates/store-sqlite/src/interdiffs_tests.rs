use mergegate_core::item::{FileMode, ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};
use mergegate_core::viewed::ViewedStore;

use super::*;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    let name = ProjectName::parse("app").unwrap();
    store
        .insert_project(&Project {
            name: name.clone(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: None,
            bots: Vec::new(),
        })
        .unwrap();
    store
        .insert_item(NewItem {
            project: name,
            branch: "login".into(),
            title: "Login".into(),
            state: ItemState::NeedsReview,
            backend: ItemBackend::Local,
            target: "main".into(),
            revision: revision(Vec::new(), &[]),
            priority: None,
            note: None,
        })
        .unwrap();
    (dir, store)
}

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn revision(interdiffs: Vec<NewInterdiff>, viewed: &[&str]) -> NewRevision {
    NewRevision {
        head: "a".repeat(40),
        base: "0".repeat(40),
        files: Vec::new(),
        guide: None,
        interdiffs,
        viewed: viewed.iter().map(|p| (*p).into()).collect(),
        parent: None,
    }
}

fn file(path: &str, old: Option<&str>, change: Change) -> InterdiffFile {
    InterdiffFile {
        stat: FileStat {
            file: ChangedFile {
                path: path.into(),
                old_path: None,
                old_blob: old.map(Into::into),
                new_blob: Some(format!("{path} new")),
                old_mode: old.map(|_| FileMode::Regular),
                new_mode: Some(FileMode::Executable),
            },
            additions: Some(1),
            deletions: old.map(|_| 1),
            generated: false,
            triage: Default::default(),
            placement: None,
        },
        change,
    }
}

fn add(store: &mut Store, revision: NewRevision) -> Result<(), StoreError> {
    store
        .add_revision(&id(), "Login", ItemState::NeedsReview, revision)
        .map(drop)
}

#[test]
fn round_trips_an_interdiff_in_path_order() {
    let (_dir, mut store) = store();
    let files = vec![
        file("src/api.rs", Some("api"), Change::Dropped),
        file("src/db.rs", None, Change::Unseparated),
        file("src/lib.rs", Some("lib"), Change::Changed),
        file("src/login.rs", None, Change::Unchanged),
    ];
    let interdiff = NewInterdiff {
        from: 1,
        files: files.clone(),
    };
    add(&mut store, revision(vec![interdiff], &[])).unwrap();

    assert_eq!(store.interdiff(&id(), 1, 2).unwrap(), Some(files));
    assert_eq!(store.interdiffs_to(&id(), 2).unwrap(), [1]);
    assert_eq!(store.interdiffs_to(&id(), 1).unwrap(), Vec::<u32>::new());
}

#[test]
fn an_empty_interdiff_differs_from_none() {
    let (_dir, mut store) = store();
    add(&mut store, revision(Vec::new(), &[])).unwrap();
    let empty = NewInterdiff {
        from: 2,
        files: Vec::new(),
    };
    add(&mut store, revision(vec![empty], &[])).unwrap();

    assert_eq!(store.interdiff(&id(), 1, 2).unwrap(), None);
    assert_eq!(store.interdiff(&id(), 2, 3).unwrap(), Some(Vec::new()));
    assert_eq!(store.interdiffs_to(&id(), 3).unwrap(), [2]);
    let other = ItemId::parse("app#2").unwrap();
    assert_eq!(store.interdiff(&other, 2, 3).unwrap(), None);
}

#[test]
fn a_revision_starts_with_its_carried_marks() {
    let (_dir, mut store) = store();
    add(
        &mut store,
        revision(Vec::new(), &["src/login.rs", "src/db.rs"]),
    )
    .unwrap();
    let viewed: Vec<String> = store.viewed(&id(), 2).unwrap().into_iter().collect();
    assert_eq!(viewed, ["src/db.rs", "src/login.rs"]);
    assert!(store.viewed(&id(), 1).unwrap().is_empty());
}

#[test]
fn a_revision_with_a_bad_interdiff_isnt_written() {
    let (_dir, mut store) = store();
    let bad = NewInterdiff {
        from: 5,
        files: vec![file("src/lib.rs", None, Change::Changed)],
    };
    assert!(add(&mut store, revision(vec![bad], &[])).is_err());
    assert_eq!(store.item(&id()).unwrap().unwrap().revision.number, 1);
    let rows: i64 = store
        .conn
        .query_row("SELECT count(*) FROM interdiff_files", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 0);
}
