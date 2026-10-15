use super::*;
use crate::fake::FakeStore;
use crate::item::{ItemBackend, ItemState, NewItem, NewRevision};
use crate::project::ProjectName;

fn add(store: &mut FakeStore) -> ItemId {
    let item = NewItem {
        project: ProjectName::parse("app").unwrap(),
        branch: "login".into(),
        title: "Login".into(),
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
        },
        priority: Some(Priority::P1),
        note: Some("blocks auth".into()),
    };
    store.insert_item(item).unwrap().id
}

fn request(id: &ItemId, priority: Option<Priority>, note: Option<&str>) -> Prioritize {
    Prioritize {
        id: id.clone(),
        priority,
        note: note.map(Into::into),
    }
}

#[test]
fn names_round_trip_and_sort_p0_first() {
    for priority in Priority::ALL {
        assert_eq!(Priority::parse(priority.as_str()), Some(priority));
    }
    assert_eq!(Priority::parse("P0"), None);
    assert_eq!(Priority::parse("p4"), None);
    let ranks: Vec<_> = [Some(Priority::P0), Some(Priority::P3), None]
        .into_iter()
        .map(rank)
        .collect();
    assert_eq!(ranks, [0, 3, 4]);
}

#[test]
fn sets_priority_and_note_together() {
    let mut store = FakeStore::default();
    let id = add(&mut store);

    let item = prioritize(
        &mut store,
        request(&id, Some(Priority::P0), Some("  now  ")),
    )
    .unwrap();
    assert_eq!(
        (item.priority, item.note.as_deref()),
        (Some(Priority::P0), Some("now"))
    );
    let stored = store.item(&id).unwrap().unwrap();
    assert_eq!(stored, item);

    // A write without a note drops the one that explained the old priority.
    let item = prioritize(&mut store, request(&id, Some(Priority::P2), None)).unwrap();
    assert_eq!((item.priority, item.note), (Some(Priority::P2), None));

    let item = prioritize(&mut store, request(&id, None, Some(" "))).unwrap();
    assert_eq!((item.priority, item.note.as_deref()), (None, None));
    assert_eq!(store.item(&id).unwrap().unwrap(), item);
}

#[test]
fn refuses_closed_and_unknown_items() {
    let mut store = FakeStore::default();
    let id = add(&mut store);
    store.set_state(&id, ItemState::Merged);
    let closed = prioritize(&mut store, request(&id, Some(Priority::P0), None));
    assert!(matches!(closed, Err(ItemError::Closed(_))));
    assert_eq!(
        store.item(&id).unwrap().unwrap().priority,
        Some(Priority::P1)
    );

    let unknown = ItemId::parse("app#9").unwrap();
    let missing = prioritize(&mut store, request(&unknown, None, None));
    assert!(matches!(missing, Err(ItemError::NoItem(_))));
}
