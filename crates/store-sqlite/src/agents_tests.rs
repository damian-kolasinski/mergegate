use mergegate_core::external::{ExternalStore, NewExternal};
use mergegate_core::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use mergegate_core::github::PullCommentKind;
use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectStore};
use mergegate_core::review::{NewReview, ReviewStore, Verdict};

use super::*;

/// `app#1` and `app#2`, one revision each.
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
    for branch in ["one", "two"] {
        store
            .insert_item(NewItem {
                project: name.clone(),
                branch: branch.into(),
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
            })
            .unwrap();
    }
    (dir, store)
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn review(store: &mut Store, item: &str) {
    let review = NewReview {
        item: id(item),
        revision: 1,
        verdict: Verdict::Comment,
        summary: String::new(),
        moved: Vec::new(),
    };
    store.insert_review(review, ItemState::NeedsReview).unwrap();
}

fn waking(command: &str) -> Agent {
    Agent {
        worktree: Some("/work".into()),
        wake: Some(command.into()),
    }
}

fn due(store: &Store) -> Vec<(String, String, u64)> {
    store
        .due_wakes()
        .unwrap()
        .into_iter()
        .map(|w| (w.item.to_string(), w.command, w.cursor))
        .collect()
}

#[test]
fn keeps_one_agent_per_item() {
    let (_dir, mut store) = store();
    assert_eq!(store.agent(&id("app#1")).unwrap(), Agent::default());
    store.set_agent(&id("app#1"), &waking("a")).unwrap();
    let moved = Agent {
        worktree: Some("/elsewhere".into()),
        wake: None,
    };
    store.set_agent(&id("app#1"), &moved).unwrap();
    assert_eq!(store.agent(&id("app#1")).unwrap(), moved);
    assert_eq!(store.agent(&id("app#2")).unwrap(), Agent::default());
}

#[test]
fn a_wake_is_due_for_events_its_agent_has_neither_read_nor_been_woken_for() {
    let (_dir, mut store) = store();
    store.set_agent(&id("app#1"), &waking("a")).unwrap();
    store
        .set_agent(
            &id("app#2"),
            &Agent {
                worktree: Some("/work".into()),
                wake: None,
            },
        )
        .unwrap();
    assert_eq!(due(&store), [], "no events yet");
    review(&mut store, "app#1");
    review(&mut store, "app#2");
    assert_eq!(due(&store), [("app#1".into(), "a".into(), 1)]);

    store.set_woken(&id("app#1"), 1).unwrap();
    assert_eq!(due(&store), []);
    review(&mut store, "app#1");
    assert_eq!(due(&store), [("app#1".into(), "a".into(), 3)]);
    // Reading the events settles them too.
    store.set_delivered(&id("app#1"), 3).unwrap();
    assert_eq!(due(&store), []);
}

#[test]
fn a_closed_items_last_events_still_wake_its_agent() {
    let (_dir, mut store) = store();
    store.set_agent(&id("app#1"), &waking("a")).unwrap();
    let merged = Notice {
        revision: 1,
        source: Source::Operator,
        kind: NoticeKind::Merged {
            commit: "b".repeat(40),
        },
    };
    store
        .notify(&id("app#1"), ItemState::Merged, merged)
        .unwrap();
    assert_eq!(due(&store), [("app#1".into(), "a".into(), 1)]);
}

#[test]
fn the_agents_own_events_never_wake_it() {
    let (_dir, mut store) = store();
    store.set_agent(&id("app#1"), &waking("a")).unwrap();
    review(&mut store, "app#1");
    let abandoned = Notice {
        revision: 1,
        source: Source::Agent,
        kind: NoticeKind::Abandoned { reason: None },
    };
    store
        .notify(&id("app#1"), ItemState::Abandoned, abandoned)
        .unwrap();
    // Woken for the operator's review, not past it for its own abandon.
    assert_eq!(due(&store), [("app#1".into(), "a".into(), 1)]);
    store.set_woken(&id("app#1"), 1).unwrap();
    assert_eq!(due(&store), []);
}

#[test]
fn a_batch_of_external_comments_wakes_the_agent() {
    let (_dir, mut store) = store();
    store.set_agent(&id("app#1"), &waking("a")).unwrap();
    let comment = NewExternal {
        item: id("app#1"),
        revision: 1,
        anchor: None,
        body: "Why?".into(),
        kind: PullCommentKind::IssueComment,
        github_id: 1,
        login: "ana".into(),
        bot: false,
        url: "https://github.com/acme/app/pull/1#c1".into(),
        path: None,
    };
    store.insert_external(comment, 1).unwrap();
    // A held comment doesn't wake it; the batch does.
    assert_eq!(due(&store), []);
    store.release(&id("app#1"), 1).unwrap();
    assert_eq!(due(&store), [("app#1".into(), "a".into(), 1)]);
}
