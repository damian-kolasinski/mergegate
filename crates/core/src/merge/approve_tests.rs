use super::*;
use crate::fake::{FakeBackend, FakeGit, FakeStore, commit, remapped};
use crate::feedback::{EventKind, Notice, NoticeKind, Source};
use crate::git::GitError;
use crate::merge::Landing;
use crate::project::{Backend, ProjectName};
use crate::review::{ReviewError, Side, WriteDraft, write_draft};

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn approving(revision: u32, summary: &str) -> Approve {
    Approve {
        id: id("app#1"),
        revision,
        summary: summary.into(),
    }
}

fn kinds(store: &FakeStore) -> Vec<EventKind> {
    let events = store.events(&id("app#1"), 0).unwrap();
    events.into_iter().map(|e| e.kind).collect()
}

fn notice(revision: u32, kind: NoticeKind) -> EventKind {
    EventKind::Notice(Notice {
        revision,
        source: Source::Operator,
        kind,
    })
}

#[test]
fn records_the_approval_then_closes_the_item_as_merged() {
    let mut store = FakeStore::with_item(1);
    let backend = FakeBackend::new(Backend::Local);
    let merge = approve(
        &mut store,
        &FakeGit::default(),
        &[&backend],
        approving(1, " Ship it.\n"),
    )
    .unwrap();

    assert_eq!(merge.item.state, ItemState::Merged);
    assert_eq!(merge.landing, Landing::Merged { commit: commit(9) });
    assert_eq!(
        backend.landed(),
        [("app#1".to_owned(), "Add login (app#1)".to_owned())]
    );
    let [review] = store.reviews() else {
        panic!("{:?}", store.reviews());
    };
    assert_eq!(review.verdict, Verdict::Approved);
    assert_eq!(review.summary, "Ship it.");
    let merged = NoticeKind::Merged { commit: commit(9) };
    assert_eq!(
        kinds(&store),
        [
            EventKind::ReviewSubmitted(review.clone()),
            notice(1, merged)
        ]
    );
}

#[test]
fn a_merged_item_keeps_only_its_newest_revision_pinned() {
    let mut store = FakeStore::with_item(2);
    let git = FakeGit::default();
    let project = ProjectName::parse("app").unwrap();
    git.pin(&project, "refs/mergegate/items/1/r1", &commit(2))
        .unwrap();
    let backend = FakeBackend::new(Backend::Local);
    approve(&mut store, &git, &[&backend], approving(2, "")).unwrap();
    assert_eq!(
        git.pins().into_iter().collect::<Vec<_>>(),
        [("refs/mergegate/items/1/r2".to_owned(), commit(3))]
    );
}

#[test]
fn a_merged_items_branch_leaves_staging() {
    let mut store = FakeStore::with_item(2);
    let git = FakeGit::default();
    let project = ProjectName::parse("app").unwrap();
    git.push("app", "login", &commit(3));
    let backend = FakeBackend::new(Backend::Local);
    approve(&mut store, &git, &[&backend], approving(2, "")).unwrap();
    assert_eq!(git.branch_head(&project, "login").unwrap(), None);
}

#[test]
fn a_conflict_hands_the_item_back_to_its_agent() {
    let mut store = FakeStore::with_item(1);
    let conflict = Landing::Conflict {
        target_commit: commit(7),
        paths: vec!["README.md".into()],
    };
    let backend = FakeBackend::new(Backend::Local).then(Ok(conflict.clone()));
    let merge = approve(
        &mut store,
        &FakeGit::default(),
        &[&backend],
        approving(1, ""),
    )
    .unwrap();

    assert_eq!(merge.item.state, ItemState::ChangesRequested);
    assert_eq!(merge.landing, conflict);
    let kind = NoticeKind::MergeConflict {
        target_commit: commit(7),
        paths: vec!["README.md".into()],
    };
    assert_eq!(kinds(&store).last(), Some(&notice(1, kind)));
}

#[test]
fn a_failure_to_land_holds_the_item_in_merging_with_gits_reason() {
    let mut store = FakeStore::with_item(1);
    let failure = GitError::Failed("the checkout of main has changes".into());
    let backend = FakeBackend::new(Backend::Local).then(Err(failure));
    let merge = approve(
        &mut store,
        &FakeGit::default(),
        &[&backend],
        approving(1, ""),
    )
    .unwrap();

    assert_eq!(merge.item.state, ItemState::Merging);
    let reason = "the checkout of main has changes".to_owned();
    assert_eq!(
        merge.landing,
        Landing::Held {
            reason: reason.clone()
        }
    );
    assert_eq!(
        kinds(&store).last(),
        Some(&notice(1, NoticeKind::MergeHeld { reason }))
    );
}

#[test]
fn only_the_newest_revision_of_an_open_item_merges() {
    let mut store = FakeStore::with_item(2);
    let git = FakeGit::default();
    let backend = FakeBackend::new(Backend::Local);
    let stale = approve(&mut store, &git, &[&backend], approving(1, "")).unwrap_err();
    assert!(
        matches!(
            stale,
            MergeError::Stale {
                revision: 1,
                newest: 2,
                ..
            }
        ),
        "{stale:?}"
    );

    store.set_state(&id("app#1"), ItemState::Merging);
    let merging = approve(&mut store, &git, &[&backend], approving(2, "")).unwrap_err();
    assert!(matches!(merging, MergeError::Merging(_)), "{merging:?}");

    store.set_state(&id("app#1"), ItemState::Abandoned);
    let closed = approve(&mut store, &git, &[&backend], approving(2, "")).unwrap_err();
    assert!(matches!(closed, MergeError::Closed(_)), "{closed:?}");

    let missing = Approve {
        id: id("app#9"),
        ..approving(1, "")
    };
    let none = approve(&mut store, &git, &[&backend], missing).unwrap_err();
    assert!(matches!(none, MergeError::NoItem(_)), "{none:?}");
    assert!(backend.landed().is_empty());
    assert!(store.reviews().is_empty());
}

#[test]
fn a_draft_that_cant_move_holds_the_approval() {
    let (mut store, git) = remapped(false);
    let draft = WriteDraft {
        id: id("app#1"),
        revision: 1,
        path: "src/changed.rs".into(),
        side: Side::New,
        start_line: 2,
        line: 2,
        body: "Why?".into(),
    };
    let draft = write_draft(&mut store, &git, draft).unwrap();
    let backend = FakeBackend::new(Backend::Local);
    let held = approve(&mut store, &git, &[&backend], approving(2, "")).unwrap_err();
    let MergeError::Review(ReviewError::Unplaced { drafts, .. }) = held else {
        panic!("{held:?}");
    };
    assert_eq!(drafts, [draft.id]);
    assert!(backend.landed().is_empty());
    assert!(store.reviews().is_empty());
    assert_eq!(
        store.item(&id("app#1")).unwrap().unwrap().state,
        ItemState::NeedsReview
    );
}

#[test]
fn another_backends_item_is_refused_before_anything_is_recorded() {
    let mut store = FakeStore::with_item(1);
    let backend = FakeBackend::new(Backend::Github);
    let error = approve(
        &mut store,
        &FakeGit::default(),
        &[&backend],
        approving(1, ""),
    )
    .unwrap_err();
    assert!(matches!(error, MergeError::NoBackend { .. }), "{error:?}");
    assert!(store.reviews().is_empty());
    let item = store.item(&id("app#1")).unwrap().unwrap();
    assert_eq!(item.state, ItemState::NeedsReview);
}
