use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};
use mergegate_core::review::{NewReview, ReviewStore, Verdict};

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

fn review(item: &str, revision: u32, summary: &str) -> NewReview {
    NewReview {
        item: id(item),
        revision,
        verdict: Verdict::ChangesRequested,
        summary: summary.into(),
        moved: Vec::new(),
        carried_from: None,
    }
}

#[test]
fn cursors_are_hub_wide_and_events_per_item() {
    let (_dir, mut store) = store();
    let (app, web) = (id("app#1"), id("web#1"));
    for (item, summary) in [("app#1", "a1"), ("web#1", "w1"), ("app#1", "a2")] {
        store
            .insert_review(review(item, 1, summary), ItemState::ChangesRequested)
            .unwrap();
    }
    let cursors = |item| -> Vec<u64> {
        store
            .events(item, 0)
            .unwrap()
            .iter()
            .map(|e| e.cursor)
            .collect()
    };
    assert_eq!(cursors(&app), [1, 3]);
    assert_eq!(cursors(&web), [2]);
    assert_eq!(store.events(&app, 1).unwrap().len(), 1);
    assert_eq!(store.last_cursor(&web).unwrap(), 2);
    assert_eq!(store.last_cursor(&id("app#9")).unwrap(), 0);
}

#[test]
fn finds_every_item_with_events_after_a_cursor() {
    let (_dir, mut store) = store();
    assert_eq!(store.newest_cursor().unwrap(), 0);
    assert!(store.logged_since(0).unwrap().is_empty());
    for (item, summary) in [("web#1", "w1"), ("app#1", "a1"), ("web#1", "w2")] {
        store
            .insert_review(review(item, 1, summary), ItemState::ChangesRequested)
            .unwrap();
    }
    assert_eq!(store.newest_cursor().unwrap(), 3);
    assert_eq!(store.logged_since(0).unwrap(), [id("app#1"), id("web#1")]);
    assert_eq!(store.logged_since(2).unwrap(), [id("web#1")]);
    assert!(store.logged_since(3).unwrap().is_empty());
}

#[test]
fn keeps_a_delivered_cursor_per_item() {
    let (_dir, mut store) = store();
    let (app, web) = (id("app#1"), id("web#1"));
    assert_eq!(store.delivered(&app).unwrap(), 0);
    store.set_delivered(&app, 3).unwrap();
    assert_eq!(store.delivered(&app).unwrap(), 3);
    assert_eq!(store.delivered(&web).unwrap(), 0);
    assert_eq!(store.delivered(&id("app#9")).unwrap(), 0);
}

#[test]
fn a_notice_moves_the_item_and_reads_back_after_reviews() {
    let (_dir, mut store) = store();
    let app = id("app#1");
    store
        .insert_review(review("app#1", 2, "Fix it"), ItemState::ChangesRequested)
        .unwrap();
    let notice = |reason: Option<&str>| Notice {
        revision: 2,
        source: Source::Agent,
        kind: NoticeKind::Abandoned {
            reason: reason.map(Into::into),
        },
    };
    let event = store
        .notify(&app, ItemState::Abandoned, notice(Some("Superseded")))
        .unwrap();
    assert_eq!(event.cursor, store.last_cursor(&app).unwrap());
    assert_eq!(event.kind, EventKind::Notice(notice(Some("Superseded"))));
    assert_eq!(
        store.item(&app).unwrap().unwrap().state,
        ItemState::Abandoned
    );

    let events = store.events(&app, 0).unwrap();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0].kind, EventKind::ReviewSubmitted(_)));
    assert_eq!(events[1], event);

    let web = id("web#1");
    let blank = store
        .notify(&web, ItemState::Abandoned, notice(None))
        .unwrap();
    assert_eq!(store.events(&web, 0).unwrap(), [blank]);
}

#[test]
fn every_merge_notice_reads_back_as_written() {
    let (_dir, mut store) = store();
    let app = id("app#1");
    store
        .insert_review(
            NewReview {
                verdict: Verdict::Approved,
                ..review("app#1", 2, "")
            },
            ItemState::Merging,
        )
        .unwrap();
    let kinds = [
        (
            ItemState::Merging,
            NoticeKind::MergeHeld {
                reason: "main has changes".into(),
            },
        ),
        (ItemState::NeedsReview, NoticeKind::MergeCancelled),
        (
            ItemState::ChangesRequested,
            NoticeKind::MergeConflict {
                target_commit: "b".repeat(40),
                paths: vec!["README.md".into(), "src/a b.rs".into()],
            },
        ),
        (
            ItemState::ChangesRequested,
            NoticeKind::MergeConflict {
                target_commit: "b".repeat(40),
                paths: Vec::new(),
            },
        ),
        (
            ItemState::Merged,
            NoticeKind::Merged {
                commit: "c".repeat(40),
            },
        ),
    ];
    let mut written = Vec::new();
    for (state, kind) in kinds {
        let notice = Notice {
            revision: 2,
            source: Source::Operator,
            kind,
        };
        written.push(store.notify(&app, state, notice).unwrap());
        assert_eq!(store.item(&app).unwrap().unwrap().state, state);
    }
    let events = store.events(&app, 0).unwrap();
    let EventKind::ReviewSubmitted(approval) = &events[0].kind else {
        panic!("{:?}", events[0]);
    };
    assert_eq!(approval.verdict, Verdict::Approved);
    assert_eq!(events[1..], written);
}

#[test]
fn a_close_on_github_reads_back_as_github_s() {
    let (_dir, mut store) = store();
    let kinds = [
        (
            "app#1",
            ItemState::Merged,
            NoticeKind::Merged {
                commit: "c".repeat(40),
            },
        ),
        (
            "web#1",
            ItemState::Abandoned,
            NoticeKind::Abandoned {
                reason: Some("Closed on GitHub".into()),
            },
        ),
    ];
    for (item, state, kind) in kinds {
        let notice = Notice {
            revision: 1,
            source: Source::External,
            kind,
        };
        let written = store.notify(&id(item), state, notice).unwrap();
        assert_eq!(written.source(), Source::External);
        assert_eq!(store.events(&id(item), 0).unwrap(), [written]);
    }
}

#[test]
fn a_notice_for_an_unknown_item_changes_nothing() {
    let (_dir, mut store) = store();
    let notice = Notice {
        revision: 1,
        source: Source::Operator,
        kind: NoticeKind::Abandoned { reason: None },
    };
    let err = store
        .notify(&id("app#9"), ItemState::Abandoned, notice)
        .unwrap_err();
    assert_eq!(err.to_string(), "no item app#9");
    assert_eq!(store.last_cursor(&id("app#1")).unwrap(), 0);
}
