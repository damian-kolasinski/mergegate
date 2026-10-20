use super::*;
use crate::fake::FakeStore;
use crate::item::ItemState;

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn spent(revision: u32, ms: u64) -> Spent {
    Spent {
        id: id("app#1"),
        revision,
        ms,
    }
}

fn item(store: &FakeStore) -> Item {
    store.item(&id("app#1")).unwrap().unwrap()
}

#[test]
fn adds_up_each_revisions_time() {
    let mut store = FakeStore::with_item(2);
    record(&mut store, spent(1, 30_000)).unwrap();
    record(&mut store, spent(1, 12_000)).unwrap();
    record(&mut store, spent(2, 5_000)).unwrap();

    let stats = stats(&store, &item(&store)).unwrap();

    assert_eq!(stats.review_ms, 47_000);
    let per: Vec<_> = stats
        .revisions
        .iter()
        .map(|r| (r.number, r.review_ms))
        .collect();
    assert_eq!(per, [(1, 42_000), (2, 5_000)]);
}

#[test]
fn lists_revisions_never_reviewed_with_none() {
    let mut store = FakeStore::with_item(2);
    record(&mut store, spent(2, 5_000)).unwrap();

    let stats = stats(&store, &item(&store)).unwrap();

    let per: Vec<_> = stats
        .revisions
        .iter()
        .map(|r| (r.number, r.review_ms))
        .collect();
    assert_eq!(per, [(1, 0), (2, 5_000)]);
}

#[test]
fn stamps_the_item_open_and_closed() {
    let mut store = FakeStore::with_item(1);
    let open = stats(&store, &item(&store)).unwrap();
    assert_eq!(open.opened_at, item(&store).created_at);
    assert_eq!(open.closed_at, None);
    assert_eq!(
        open.revisions[0].submitted_at,
        item(&store).revision.submitted_at
    );

    store.set_state(&id("app#1"), ItemState::Merged);

    let closed = stats(&store, &item(&store)).unwrap();
    assert!(closed.closed_at.is_some());
}

#[test]
fn refuses_time_on_a_closed_item() {
    let mut store = FakeStore::with_item(1);
    record(&mut store, spent(1, 1_000)).unwrap();
    store.set_state(&id("app#1"), ItemState::Merged);

    let refused = record(&mut store, spent(1, 1_000));

    assert!(matches!(refused, Err(ReviewTimeError::Closed(_))));
    assert_eq!(stats(&store, &item(&store)).unwrap().review_ms, 1_000);
}

#[test]
fn refuses_a_revision_the_item_lacks() {
    let mut store = FakeStore::with_item(1);

    for revision in [0, 2] {
        let refused = record(&mut store, spent(revision, 1_000));
        assert!(matches!(refused, Err(ReviewTimeError::NoRevision { .. })));
    }
}

#[test]
fn refuses_an_unknown_item() {
    let mut store = FakeStore::with_item(1);

    let refused = record(
        &mut store,
        Spent {
            id: id("app#9"),
            revision: 1,
            ms: 1_000,
        },
    );

    assert!(matches!(refused, Err(ReviewTimeError::NoItem(_))));
}

#[test]
fn refuses_a_report_longer_than_a_client_could_count() {
    let mut store = FakeStore::with_item(1);

    let refused = record(&mut store, spent(1, MAX_REPORT_MS + 1));

    assert!(matches!(refused, Err(ReviewTimeError::TooLong { .. })));
    assert_eq!(stats(&store, &item(&store)).unwrap().review_ms, 0);
}
