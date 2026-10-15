use super::*;
use crate::fake::{FakeGit, FakeStore, commit};
use crate::item::{ItemBackend, ItemState, NewItem, NewRevision};

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

fn add(store: &mut FakeStore, branch: &str, head: u8, target: &str) -> ItemId {
    let item = NewItem {
        project: app(),
        branch: branch.into(),
        title: format!("Work on {branch}"),
        state: ItemState::NeedsReview,
        backend: ItemBackend::Local,
        target: target.into(),
        revision: NewRevision {
            head: commit(head),
            base: commit(1),
            files: Vec::new(),
            guide: None,
            interdiffs: Vec::new(),
            viewed: Vec::new(),
        },
        priority: None,
        note: None,
    };
    store.insert_item(item).unwrap().id
}

/// A whole pass, the store held throughout.
fn pairs(
    store: &mut FakeStore,
    git: &FakeGit,
    project: &ProjectName,
) -> Result<bool, ConflictError> {
    let tried = Pairing::read(store, project)?.try_all(git)?;
    save_pairs(store, project, &tried)
}

fn shown(store: &FakeStore) -> BTreeMap<ItemId, Vec<ItemId>> {
    conflicting(
        &store.open_items(None).unwrap(),
        &store.pairs(None).unwrap(),
    )
}

#[test]
fn tries_each_pair_with_one_target_once_per_heads() {
    let mut store = FakeStore::default();
    let git = FakeGit::default();
    let a = add(&mut store, "a", 2, "main");
    let b = add(&mut store, "b", 3, "main");
    let c = add(&mut store, "c", 4, "main");
    add(&mut store, "d", 5, "release");
    store.set_state(&c, ItemState::Merging);
    git.conflict_between(&commit(2), &commit(4), &["src/login.rs"]);

    assert!(pairs(&mut store, &git, &app()).unwrap());
    assert_eq!(git.merged().len(), 3);
    assert_eq!(
        shown(&store),
        BTreeMap::from([(a.clone(), vec![c.clone()]), (c.clone(), vec![a.clone()])])
    );

    // Nothing moved: nothing merged, nothing to show.
    assert!(!pairs(&mut store, &git, &app()).unwrap());
    assert_eq!(git.merged().len(), 3);

    // b's new head is tried against a and c only.
    store
        .add_revision(
            &b,
            "Work on b",
            ItemState::NeedsReview,
            NewRevision {
                head: commit(6),
                base: commit(1),
                files: Vec::new(),
                guide: None,
                interdiffs: Vec::new(),
                viewed: Vec::new(),
            },
        )
        .unwrap();
    git.conflict_between(&commit(6), &commit(4), &["src/b.rs"]);
    assert!(pairs(&mut store, &git, &app()).unwrap());
    assert_eq!(git.merged().len(), 5);
    assert_eq!(shown(&store)[&c], [a.clone(), b.clone()]);
}

#[test]
fn a_closed_item_conflicts_with_nothing() {
    let mut store = FakeStore::default();
    let git = FakeGit::default();
    let a = add(&mut store, "a", 2, "main");
    add(&mut store, "b", 3, "main");
    git.conflict_between(&commit(2), &commit(3), &["x"]);
    pairs(&mut store, &git, &app()).unwrap();

    // Before the next pass, the cached pair is stale but shows nothing.
    store.set_state(&a, ItemState::Merged);
    assert!(shown(&store).is_empty());
    // The pass drops it without changing what shows.
    assert!(!pairs(&mut store, &git, &app()).unwrap());
    assert!(store.pairs(None).unwrap().is_empty());
}

#[test]
fn a_stale_pair_shows_nothing() {
    let mut store = FakeStore::default();
    let git = FakeGit::default();
    let a = add(&mut store, "a", 2, "main");
    add(&mut store, "b", 3, "main");
    git.conflict_between(&commit(2), &commit(3), &["x"]);
    pairs(&mut store, &git, &app()).unwrap();
    store
        .add_revision(
            &a,
            "Work on a",
            ItemState::NeedsReview,
            NewRevision {
                head: commit(4),
                base: commit(1),
                files: Vec::new(),
                guide: None,
                interdiffs: Vec::new(),
                viewed: Vec::new(),
            },
        )
        .unwrap();
    assert!(shown(&store).is_empty());
}
