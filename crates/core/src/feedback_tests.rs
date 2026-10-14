use super::*;
use crate::fake::FakeStore;
use crate::item::ItemState;
use crate::review::{SubmitReview, Verdict, submit};

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn review(store: &mut FakeStore, summary: &str) -> u64 {
    let request = SubmitReview {
        id: id("app#1"),
        revision: 1,
        verdict: Verdict::ChangesRequested,
        summary: summary.into(),
    };
    submit(store, request).unwrap();
    store.last_cursor(&id("app#1")).unwrap()
}

fn summaries(feedback: &Feedback) -> Vec<&str> {
    feedback
        .events
        .iter()
        .map(|e| match &e.kind {
            EventKind::ReviewSubmitted(review) => review.summary.as_str(),
            kind => panic!("not a review: {kind:?}"),
        })
        .collect()
}

#[test]
fn returns_what_wasnt_acknowledged_until_it_is() {
    let mut store = FakeStore::with_item(1);
    let app = id("app#1");
    let none = feedback(&store, &app, None).unwrap();
    assert!(none.events.is_empty());
    assert_eq!(none.cursor, 0);

    let first = review(&mut store, "First");
    let read = feedback(&store, &app, None).unwrap();
    assert_eq!(summaries(&read), ["First"]);
    assert_eq!(read.cursor, first);
    assert_eq!(read.item.state, ItemState::ChangesRequested);
    // Not acknowledged: the next read returns it again.
    assert_eq!(feedback(&store, &app, None).unwrap(), read);

    assert_eq!(acknowledge(&mut store, &app, first).unwrap(), first);
    let after = feedback(&store, &app, None).unwrap();
    assert!(after.events.is_empty());
    assert_eq!(after.cursor, first);

    let second = review(&mut store, "Second");
    let read = feedback(&store, &app, None).unwrap();
    assert_eq!(summaries(&read), ["Second"]);
    assert_eq!(read.cursor, second);
}

#[test]
fn since_replays_from_any_cursor_without_moving_the_delivered_one() {
    let mut store = FakeStore::with_item(1);
    let app = id("app#1");
    let first = review(&mut store, "First");
    let second = review(&mut store, "Second");
    acknowledge(&mut store, &app, second).unwrap();

    assert_eq!(
        summaries(&feedback(&store, &app, Some(0)).unwrap()),
        ["First", "Second"]
    );
    assert_eq!(
        summaries(&feedback(&store, &app, Some(first)).unwrap()),
        ["Second"]
    );
    // Acknowledging a replay's older cursor keeps the newer one.
    let delivered = store.delivered(&app).unwrap();
    assert_eq!(acknowledge(&mut store, &app, first).unwrap(), delivered);
    assert!(feedback(&store, &app, None).unwrap().events.is_empty());
}

#[test]
fn refuses_cursors_past_the_newest_event() {
    let mut store = FakeStore::with_item(1);
    let app = id("app#1");
    let first = review(&mut store, "First");
    let err = acknowledge(&mut store, &app, first + 1).unwrap_err();
    assert_eq!(err.to_string(), format!("app#1 has no event {}", first + 1));
    let err = acknowledge(&mut store, &id("app#2"), 0).unwrap_err();
    assert_eq!(err.to_string(), "no item app#2");
    assert!(matches!(
        feedback(&store, &id("app#2"), None),
        Err(FeedbackError::NoItem(_))
    ));
}

#[test]
fn a_wait_for_the_operator_ends_on_an_operator_event() {
    let mut store = FakeStore::with_item(1);
    let app = id("app#1");
    let events = feedback(&store, &app, None).unwrap().events;
    assert!(!Wake::Any.by(&events));
    assert!(!Wake::Operator.by(&events));
    review(&mut store, "First");
    let events = feedback(&store, &app, None).unwrap().events;
    assert!(Wake::Any.by(&events));
    assert!(Wake::Operator.by(&events));
}
