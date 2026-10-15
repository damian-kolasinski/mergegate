use super::*;
use crate::fake::{FakeGit, FakeGithub, FakeStore, commit};
use crate::item::{ItemState, Submit, submit};
use crate::project::ProjectName;

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

/// What [`FakeStore::with_item`] submitted: c1 on main, c2 on `login`.
fn git() -> FakeGit {
    let git = FakeGit::default()
        .commit(&commit(1), None, "Initial", &["README.md"])
        .commit(&commit(2), Some(&commit(1)), "Add login", &["src/login.rs"])
        .commit(&commit(3), Some(&commit(2)), "More", &["src/more.rs"])
        .upstream("/src/app", "main", &commit(1));
    git.push("app", "login", &commit(2));
    git
}

#[test]
fn drops_a_closed_items_branch_unless_it_moved_on() {
    let mut store = FakeStore::with_item(1);
    let git = git();
    drop_branch(&store, &git, &id("app#1")).unwrap();
    assert_eq!(
        git.branch_head(&app(), "login").unwrap(),
        Some(commit(2)),
        "open"
    );

    store.set_state(&id("app#1"), ItemState::Abandoned);
    // Pushed after its newest revision: work the item never saw.
    git.push("app", "login", &commit(3));
    drop_branch(&store, &git, &id("app#1")).unwrap();
    assert_eq!(git.branch_head(&app(), "login").unwrap(), Some(commit(3)));

    git.push("app", "login", &commit(2));
    drop_branch(&store, &git, &id("app#1")).unwrap();
    assert_eq!(git.branch_head(&app(), "login").unwrap(), None);
}

#[test]
fn keeps_a_branch_an_open_item_holds() {
    let mut store = FakeStore::with_item(1);
    let git = git();
    store.set_state(&id("app#1"), ItemState::Merged);
    // Submitted again from the same commit, as a new item.
    let request = Submit {
        project: "app".into(),
        branch: "login".into(),
        head: commit(2),
        title: None,
        worktree: None,
        wake: None,
        guide: None,
        priority: None,
        note: None,
    };
    let again = submit(&mut store, &git, &FakeGithub::default(), request).unwrap();
    assert_eq!(again.item.id, id("app#2"));
    drop_branch(&store, &git, &id("app#1")).unwrap();
    assert_eq!(git.branch_head(&app(), "login").unwrap(), Some(commit(2)));
}

#[test]
fn sweeps_every_closed_items_branch_and_names_those_left() {
    let mut store = FakeStore::with_item(1);
    store.set_state(&id("app#1"), ItemState::Abandoned);
    let git = git();
    assert!(drop_closed_branches(&store, &git).unwrap().is_empty());
    assert_eq!(git.branch_head(&app(), "login").unwrap(), None);

    let git = self::git().failing_drop();
    let failed = drop_closed_branches(&store, &git).unwrap();
    let [(item, e)] = failed.as_slice() else {
        panic!("{failed:?}");
    };
    assert_eq!(*item, id("app#1"));
    assert_eq!(e.to_string(), "update-ref failed");
}
