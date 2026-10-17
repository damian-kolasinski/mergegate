use super::*;
use crate::fake::FakeStore;
use crate::feedback::{EventKind, Notice, NoticeKind, Source, feedback};
use crate::item::{ItemBackend, ItemId, ItemState, NewItem, NewRevision};
use crate::project::ProjectName;

fn add(store: &mut FakeStore, branch: &str) -> ItemId {
    let item = NewItem {
        project: ProjectName::parse("app").unwrap(),
        branch: branch.into(),
        title: format!("Work on {branch}"),
        state: ItemState::NeedsReview,
        backend: ItemBackend::Local,
        target: "main".into(),
        revision: NewRevision {
            head: "1".repeat(40),
            base: "0".repeat(40),
            files: Vec::new(),
            guide: None,
            interdiffs: Vec::new(),
            viewed: Vec::new(),
            parent: None,
        },
        priority: None,
        note: None,
    };
    store.insert_item(item).unwrap().id
}

fn abandon(store: &mut FakeStore, id: &ItemId, reason: &str) -> u64 {
    let notice = Notice {
        revision: 1,
        source: Source::Operator,
        kind: NoticeKind::Abandoned {
            reason: Some(reason.into()),
        },
    };
    store
        .notify(id, ItemState::Abandoned, notice)
        .unwrap()
        .cursor
}

fn reasons(log: &Log) -> Vec<(String, &str)> {
    log.events
        .iter()
        .map(|(item, event)| match &event.kind {
            EventKind::Notice(Notice {
                kind: NoticeKind::Abandoned { reason },
                ..
            }) => (item.id.to_string(), reason.as_deref().unwrap()),
            kind => panic!("not an abandon: {kind:?}"),
        })
        .collect()
}

#[test]
fn starts_from_now_and_returns_every_items_events_in_order() {
    // app#1 is the fixture's; these are app#2 and app#3.
    let mut store = FakeStore::with_item(1);
    let a = add(&mut store, "a");
    let b = add(&mut store, "b");

    let empty = log(&store, None).unwrap();
    assert_eq!(
        empty,
        Log {
            events: Vec::new(),
            cursor: 0
        }
    );

    let first = abandon(&mut store, &a, "first");
    let now = log(&store, None).unwrap();
    assert!(now.events.is_empty());
    assert_eq!(now.cursor, first);

    abandon(&mut store, &b, "second");
    let last = abandon(&mut store, &a, "third");
    let read = log(&store, Some(first)).unwrap();
    assert_eq!(
        reasons(&read),
        [("app#3".into(), "second"), ("app#2".into(), "third")]
    );
    assert_eq!(read.cursor, last);
    assert_eq!(read.events[0].0.state, ItemState::Abandoned);

    let caught_up = log(&store, Some(last)).unwrap();
    assert!(caught_up.events.is_empty());
    assert_eq!(caught_up.cursor, last);
}

#[test]
fn acknowledges_nothing() {
    let mut store = FakeStore::with_item(1);
    let app = ItemId::parse("app#1").unwrap();
    abandon(&mut store, &app, "gone");
    assert_eq!(reasons(&log(&store, Some(0)).unwrap()).len(), 1);
    assert_eq!(store.delivered(&app).unwrap(), 0);
    assert_eq!(feedback(&store, &app, None).unwrap().events.len(), 1);
}
