use std::cell::RefCell;

use super::*;
use crate::fake::{FakeBackend, FakeGit, FakeGithub, FakeStore, commit};
use crate::feedback::Source;
use crate::git::GitError;
use crate::item::{Abandon, Submit, abandon, submit};
use crate::merge::{Approve, Landing, approve, cancel};
use crate::project::{Backend, Project, ProjectName, ProjectStore};
use crate::review::{Side, WriteDraft, write_draft};
use crate::review_time::ReviewTimeStore;

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn held() -> Result<Landing, GitError> {
    Err(GitError::Failed("the checkout of main has changes".into()))
}

fn app() -> FakeStore {
    let mut store = FakeStore::default();
    store
        .insert_project(&Project {
            name: ProjectName::parse("app").unwrap(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: None,
            bots: Vec::new(),
        })
        .unwrap();
    store
}

/// r1 (c2) adds `src/login.rs` on main at c1. Main then moves to c3, which
/// changes `README.md`; c4 is r1 rebased onto it, and c5 changes login.rs
/// on top.
fn login() -> (FakeStore, FakeGit) {
    let mut store = app();
    let git = FakeGit::default()
        .write(&commit(1), None, "Initial", &[("README.md", "readme")])
        .write(
            &commit(2),
            Some(&commit(1)),
            "Add login",
            &[("src/login.rs", "login")],
        )
        .write(
            &commit(3),
            Some(&commit(1)),
            "Upstream",
            &[("README.md", "readme 2")],
        )
        .write(
            &commit(4),
            Some(&commit(3)),
            "Add login",
            &[("src/login.rs", "login")],
        )
        .write(
            &commit(5),
            Some(&commit(4)),
            "Fix login",
            &[("src/login.rs", "login 2")],
        )
        .upstream("/src/app", "main", &commit(1));
    send(&mut store, &git, 2);
    (store, git)
}

/// r1 (c2) adds `src/login.rs` and changes `src/db.rs`, which main's c3
/// changes too, so rebasing conflicts on it. c4 is r1 rebased onto c3,
/// resolving db.rs to `db`, or to main's when that's None.
fn conflicted(db: Option<&str>) -> (FakeStore, FakeGit) {
    let mut store = app();
    let mut r2 = vec![("src/login.rs", "login")];
    r2.extend(db.map(|db| ("src/db.rs", db)));
    let git = FakeGit::default()
        .write(&commit(1), None, "Initial", &[("src/db.rs", "db")])
        .write(
            &commit(2),
            Some(&commit(1)),
            "Add login",
            &[("src/login.rs", "login"), ("src/db.rs", "db r1")],
        )
        .write(
            &commit(3),
            Some(&commit(1)),
            "Upstream",
            &[("src/db.rs", "db 2")],
        )
        .write(&commit(4), Some(&commit(3)), "Add login", &r2)
        .upstream("/src/app", "main", &commit(1));
    send(&mut store, &git, 2);
    (store, git)
}

fn send(store: &mut FakeStore, git: &FakeGit, head: u8) -> Item {
    if head >= 4 {
        git.push_upstream("/src/app", "main", &commit(3));
    }
    git.push("app", "login", &commit(head));
    let request = Submit {
        project: "app".into(),
        branch: "login".into(),
        head: commit(head),
        title: None,
        worktree: None,
        wake: None,
        device: None,
        guide: None,
        priority: None,
        note: None,
        on: None,
    };
    submit(store, git, &FakeGithub::default(), request)
        .unwrap()
        .item
}

/// Approves r1, which the backend holds.
fn approve_held(store: &mut FakeStore, git: &FakeGit, backend: &FakeBackend) {
    let request = Approve {
        id: id(),
        revision: 1,
        summary: "Ship it.".into(),
    };
    let merge = approve(&RefCell::new(store), git, &[backend], request).unwrap();
    assert_eq!(merge.item.state, ItemState::Merging);
}

#[test]
fn a_held_approval_carries_to_the_same_diff_on_a_new_base() {
    let (mut store, git) = login();
    let backend = FakeBackend::new(Backend::Local).then(held());
    approve_held(&mut store, &git, &backend);
    let item = send(&mut store, &git, 4);
    assert_eq!(item.state, ItemState::NeedsReview);
    assert_eq!(unchanged_since_approval(&store, &item).unwrap(), Some(1));

    let carried = carry(&RefCell::new(&mut store), &git, &[&backend], &id())
        .unwrap()
        .unwrap();
    assert_eq!(carried.from, 1);
    assert_eq!(carried.merge.item.state, ItemState::Merged);
    assert_eq!(carried.merge.landing, Landing::Merged { commit: commit(9) });
    assert_eq!(backend.landed().len(), 2);
    let [_, again] = store.reviews() else {
        panic!("{:?}", store.reviews());
    };
    assert_eq!(again.revision, 2);
    assert_eq!(again.verdict, Verdict::Approved);
    assert_eq!(
        again.summary,
        "Approved again: the diff is unchanged since r1."
    );
    // The operator approved once, so their review time has one turn.
    let sent = store.sent_reviews(&id()).unwrap();
    assert_eq!(sent.len(), 1, "{sent:?}");
}

#[test]
fn a_changed_file_needs_the_operator_again() {
    let (mut store, git) = login();
    let backend = FakeBackend::new(Backend::Local).then(held());
    approve_held(&mut store, &git, &backend);
    let item = send(&mut store, &git, 5);
    assert_eq!(standing_approval(&store, &id()).unwrap(), Some(1));
    assert_eq!(unchanged_since_approval(&store, &item).unwrap(), None);
    assert_eq!(
        carry(&RefCell::new(&mut store), &git, &[&backend], &id()).unwrap(),
        None
    );
    assert_eq!(backend.landed().len(), 1);
    assert_eq!(
        store.item(&id()).unwrap().unwrap().state,
        ItemState::NeedsReview
    );
}

#[test]
fn a_cancelled_merge_takes_the_approval_back() {
    let (mut store, git) = login();
    let backend = FakeBackend::new(Backend::Local).then(held());
    approve_held(&mut store, &git, &backend);
    cancel(&mut store, &id()).unwrap();
    assert_eq!(standing_approval(&store, &id()).unwrap(), None);
    send(&mut store, &git, 4);
    assert_eq!(
        carry(&RefCell::new(&mut store), &git, &[&backend], &id()).unwrap(),
        None
    );
    assert_eq!(backend.landed().len(), 1);
}

#[test]
fn a_later_review_replaces_the_approval() {
    let (mut store, git) = login();
    let backend = FakeBackend::new(Backend::Local).then(held());
    approve_held(&mut store, &git, &backend);
    send(&mut store, &git, 4);
    let review = NewReview {
        item: id(),
        revision: 2,
        verdict: Verdict::ChangesRequested,
        summary: String::new(),
        moved: Vec::new(),
        carried_from: None,
    };
    store
        .insert_review(review, ItemState::ChangesRequested)
        .unwrap();
    assert_eq!(standing_approval(&store, &id()).unwrap(), None);
}

#[test]
fn a_draft_keeps_the_item_for_the_operator() {
    let (mut store, git) = login();
    let backend = FakeBackend::new(Backend::Local).then(held());
    approve_held(&mut store, &git, &backend);
    send(&mut store, &git, 4);
    let draft = WriteDraft {
        id: id(),
        revision: 2,
        path: "src/login.rs".into(),
        side: Side::New,
        start_line: 1,
        line: 1,
        body: "Why?".into(),
    };
    write_draft(&mut store, &git, draft).unwrap();
    assert_eq!(
        carry(&RefCell::new(&mut store), &git, &[&backend], &id()).unwrap(),
        None
    );
    assert_eq!(
        store.item(&id()).unwrap().unwrap().state,
        ItemState::NeedsReview
    );
}

#[test]
fn a_held_item_isnt_carried_until_a_new_revision_comes() {
    let (mut store, git) = login();
    let backend = FakeBackend::new(Backend::Local).then(held());
    approve_held(&mut store, &git, &backend);
    assert_eq!(
        carry(&RefCell::new(&mut store), &git, &[&backend], &id()).unwrap(),
        None
    );
    assert_eq!(backend.landed().len(), 1);
}

#[test]
fn a_file_the_rebase_conflicted_on_needs_the_operator_again() {
    // Even resolved to r1's own content: its diff against the new base isn't
    // the one approved.
    let (mut store, git) = conflicted(Some("db r1"));
    let backend = FakeBackend::new(Backend::Local).then(held());
    approve_held(&mut store, &git, &backend);
    let item = send(&mut store, &git, 4);
    let files = store.interdiff(&id(), 1, 2).unwrap().unwrap();
    let db = files.iter().find(|f| f.stat.file.path == "src/db.rs");
    assert_eq!(db.map(|f| f.change), Some(Change::Unseparated));
    assert_eq!(unchanged_since_approval(&store, &item).unwrap(), None);
    assert_eq!(
        carry(&RefCell::new(&mut store), &git, &[&backend], &id()).unwrap(),
        None
    );
    assert_eq!(backend.landed().len(), 1);
}

#[test]
fn a_conflicted_file_the_new_revision_leaves_to_main_needs_the_operator_again() {
    let (mut store, git) = conflicted(None);
    let backend = FakeBackend::new(Backend::Local).then(held());
    approve_held(&mut store, &git, &backend);
    let item = send(&mut store, &git, 4);
    // The interdiff has only the unchanged login.rs: the dropped db.rs
    // conflicted, so it's in none.
    let files = store.interdiff(&id(), 1, 2).unwrap().unwrap();
    let changes: Vec<_> = files
        .iter()
        .map(|f| (f.stat.file.path.as_str(), f.change))
        .collect();
    assert_eq!(changes, [("src/login.rs", Change::Unchanged)]);
    assert_eq!(unchanged_since_approval(&store, &item).unwrap(), None);
    assert_eq!(
        carry(&RefCell::new(&mut store), &git, &[&backend], &id()).unwrap(),
        None
    );
}

#[test]
fn abandoning_takes_the_approval_back() {
    let (mut store, git) = login();
    let backend = FakeBackend::new(Backend::Local).then(held());
    approve_held(&mut store, &git, &backend);
    let request = Abandon {
        id: id(),
        source: Source::Operator,
        reason: None,
    };
    abandon(&mut store, &git, &FakeGithub::default(), request).unwrap();
    assert_eq!(standing_approval(&store, &id()).unwrap(), None);
}

#[test]
fn a_parent_abandoned_takes_the_approval_back() {
    // The approval was of one part of work that won't land: the same diff on
    // the target is a change the operator hasn't seen.
    let (mut store, git) = login();
    let backend = FakeBackend::new(Backend::Local).then(held());
    approve_held(&mut store, &git, &backend);
    let notice = Notice {
        revision: 1,
        source: Source::Operator,
        kind: NoticeKind::ParentAbandoned {
            parent: ItemId::parse("app#9").unwrap(),
            upstream: commit(1),
        },
    };
    store
        .notify(&id(), ItemState::ChangesRequested, notice)
        .unwrap();
    assert_eq!(standing_approval(&store, &id()).unwrap(), None);
}
