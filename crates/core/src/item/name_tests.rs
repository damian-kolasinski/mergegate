use super::*;
use crate::fake::{FakeStore, commit};
use crate::github::{GithubRepo, Pull};
use crate::item::{ItemState, NewItem, NewRevision};
use crate::project::ProjectName;

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

/// Opens an item on `branch`, with pull request `pull` on GitHub if given.
fn open(store: &mut FakeStore, branch: &str, pull: Option<u64>) -> Item {
    insert(store, branch, pull, ItemState::NeedsReview)
}

fn insert(store: &mut FakeStore, branch: &str, pull: Option<u64>, state: ItemState) -> Item {
    let backend = match pull {
        Some(number) => ItemBackend::Github {
            repo: GithubRepo::parse("acme/app").unwrap(),
            pull: Pull {
                number,
                url: format!("https://github.com/acme/app/pull/{number}"),
            },
        },
        None => ItemBackend::Local,
    };
    store
        .insert_item(NewItem {
            project: ProjectName::parse("app").unwrap(),
            branch: branch.into(),
            title: branch.into(),
            state,
            backend,
            target: "main".into(),
            revision: NewRevision {
                head: commit(2),
                base: commit(1),
                files: Vec::new(),
                guide: None,
                interdiffs: Vec::new(),
                viewed: Vec::new(),
            },
            priority: None,
            note: None,
        })
        .unwrap()
}

#[test]
fn a_github_item_is_named_by_its_pull_request_and_a_local_one_by_its_id() {
    let mut store = FakeStore::default();
    let github = open(&mut store, "login", Some(84));
    let local = open(&mut store, "logout", None);
    assert_eq!(github.name(), id("app#84"));
    assert_eq!(local.name(), id("app#2"));
}

#[test]
fn finds_an_item_by_its_name_or_its_id() {
    let mut store = FakeStore::default();
    open(&mut store, "login", Some(84));
    let found = |name| named(&store, &id(name)).unwrap().map(|item| item.branch);
    assert_eq!(found("app#84").as_deref(), Some("login"));
    // An ID no item is named by still finds its item.
    assert_eq!(found("app#1").as_deref(), Some("login"));
    assert_eq!(found("app#2"), None);
    assert_eq!(found("web#84"), None);
}

#[test]
fn a_name_wins_over_the_id_it_shares() {
    let mut store = FakeStore::default();
    // app#1 is pull request #2, and app#2 is #1.
    open(&mut store, "login", Some(2));
    open(&mut store, "logout", Some(1));
    let found = |name| named(&store, &id(name)).unwrap().unwrap().branch;
    assert_eq!(found("app#2"), "login");
    assert_eq!(found("app#1"), "logout");
}

#[test]
fn a_local_item_and_a_pull_request_sharing_a_name_go_open_first_then_newest() {
    let mut store = FakeStore::default();
    // The project moved to GitHub under open item app#1, and its first pull
    // request there is #1 too.
    open(&mut store, "login", None);
    insert(&mut store, "logout", Some(1), ItemState::Merged);
    let found = |store: &FakeStore| named(store, &id("app#1")).unwrap().unwrap().branch;
    assert_eq!(found(&store), "login");
    // Both open, after another move whose first pull request is #1 again.
    open(&mut store, "signup", Some(1));
    assert_eq!(found(&store), "signup");
}
