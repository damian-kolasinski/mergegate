use super::*;
use crate::conflict::Watch;
use crate::fake::{FakeGit, FakeGithub, FakeStore, commit};
use crate::item::{ItemBackend, ItemId, ItemStore, NewItem, NewRevision};
use crate::project::{Backend, Project, ProjectStore};

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

/// `app` targets main, at c1 upstream.
fn setup() -> (FakeStore, FakeGit) {
    let mut store = FakeStore::default();
    store
        .insert_project(&Project {
            name: app(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: None,
            bots: Vec::new(),
        })
        .unwrap();
    let git = FakeGit::default().upstream("/src/app", "main", &commit(1));
    (store, git)
}

fn revision(head: u8, parent: Option<&ItemId>) -> NewRevision {
    NewRevision {
        head: commit(head),
        base: commit(1),
        files: Vec::new(),
        guide: None,
        interdiffs: Vec::new(),
        viewed: Vec::new(),
        parent: parent.cloned(),
    }
}

fn add(store: &mut FakeStore, branch: &str, head: u8, parent: Option<&ItemId>) -> ItemId {
    let item = NewItem {
        project: app(),
        branch: branch.into(),
        title: format!("Work on {branch}"),
        state: ItemState::NeedsReview,
        backend: ItemBackend::Local,
        target: "main".into(),
        revision: revision(head, parent),
        priority: None,
        note: None,
    };
    store.insert_item(item).unwrap().id
}

/// A watch pass as the hub runs one, recording what merged cleanly.
struct Hub {
    store: FakeStore,
    git: FakeGit,
    seen: Seen,
    clean: Clean,
}

impl Hub {
    fn new() -> Self {
        let (store, git) = setup();
        Self {
            store,
            git,
            seen: Seen::default(),
            clean: Clean::default(),
        }
    }

    /// Returns the items that started or stopped merging cleanly.
    fn pass(&mut self) -> Vec<ItemId> {
        let watch = Watch::read(&self.store, &app()).unwrap().unwrap();
        let mut merges = watch.try_merges(&self.git, &FakeGithub::default(), &self.seen);
        merges.hand_back(&mut self.store, &self.git).unwrap();
        self.seen = std::mem::take(&mut merges.seen);
        self.clean.record(&app(), &self.seen)
    }

    fn mergeable(&self, id: &ItemId) -> Mergeable {
        let item = self.store.item(id).unwrap().unwrap();
        mergeable(&self.store, &self.clean, &item).unwrap()
    }
}

#[test]
fn a_submitted_revision_is_unknown_until_the_watch_tries_it() {
    let mut hub = Hub::new();
    let id = add(&mut hub.store, "login", 2, None);
    assert_eq!(hub.mergeable(&id), Mergeable::Unknown);
    assert_eq!(hub.pass(), std::slice::from_ref(&id));
    assert_eq!(hub.mergeable(&id), Mergeable::Clean);
    // Nothing moved.
    assert!(hub.pass().is_empty());

    hub.store
        .add_revision(
            &id,
            "Work on login",
            ItemState::NeedsReview,
            revision(3, None),
        )
        .unwrap();
    assert_eq!(hub.mergeable(&id), Mergeable::Unknown);
    hub.pass();
    assert_eq!(hub.mergeable(&id), Mergeable::Clean);
}

#[test]
fn a_target_that_moves_into_a_conflict_reports_its_paths() {
    let mut hub = Hub::new();
    let id = add(&mut hub.store, "login", 2, None);
    let other = add(&mut hub.store, "logout", 3, None);
    hub.pass();
    assert_eq!(hub.mergeable(&id), Mergeable::Clean);

    hub.git.push_upstream("/src/app", "main", &commit(5));
    hub.git
        .conflict_between(&commit(5), &commit(2), &["src/login.rs"]);
    assert_eq!(hub.pass(), std::slice::from_ref(&id));
    let conflict = Mergeable::Conflict {
        target_commit: commit(5),
        paths: vec!["src/login.rs".into()],
    };
    assert_eq!(hub.mergeable(&id), conflict);
    assert_eq!(hub.mergeable(&other), Mergeable::Clean);

    // Its agent was told to rebase, and the watch doesn't try the revision
    // again, so the conflict stands as the target moves on.
    hub.git.push_upstream("/src/app", "main", &commit(6));
    hub.pass();
    assert_eq!(hub.mergeable(&id), conflict);
    assert_eq!(hub.mergeable(&other), Mergeable::Clean);
}

#[test]
fn a_target_that_fails_to_fetch_is_unknown() {
    let mut hub = Hub::new();
    let id = add(&mut hub.store, "login", 2, None);
    hub.pass();
    assert_eq!(hub.mergeable(&id), Mergeable::Clean);

    hub.git.delete_upstream("/src/app", "main");
    assert_eq!(hub.pass(), std::slice::from_ref(&id));
    assert_eq!(hub.mergeable(&id), Mergeable::Unknown);
}

#[test]
fn approved_and_closed_items_are_unknown() {
    let mut hub = Hub::new();
    let id = add(&mut hub.store, "login", 2, None);
    hub.pass();
    hub.store.set_state(&id, ItemState::Merging);
    assert_eq!(hub.mergeable(&id), Mergeable::Unknown);
    hub.store.set_state(&id, ItemState::Merged);
    assert_eq!(hub.mergeable(&id), Mergeable::Unknown);
}

#[test]
fn a_stacked_item_is_unknown() {
    let mut hub = Hub::new();
    let login = add(&mut hub.store, "login", 2, None);
    let profile = add(&mut hub.store, "profile", 3, Some(&login));
    hub.pass();
    assert_eq!(hub.mergeable(&login), Mergeable::Clean);
    assert_eq!(hub.mergeable(&profile), Mergeable::Unknown);
}

#[test]
fn a_removed_project_is_forgotten() {
    let mut hub = Hub::new();
    let id = add(&mut hub.store, "login", 2, None);
    hub.pass();
    hub.clean.retain(&[]);
    assert_eq!(hub.mergeable(&id), Mergeable::Unknown);
}
