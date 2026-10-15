use api::feedback::EventBody;
use api::item::ItemState;

use super::*;

fn feedback(events: &[u64], cursor: u64) -> Feedback {
    Feedback {
        item: "app#1".into(),
        state: ItemState::NeedsReview,
        events: events
            .iter()
            .map(|&cursor| Event {
                cursor,
                item: "app#1".into(),
                revision: 1,
                source: Source::Agent,
                body: EventBody::MergeCancelled {},
            })
            .collect(),
        cursor,
    }
}

#[test]
fn asks_again_from_where_the_hub_put_a_cursor_past_the_items_events() {
    // The item's newest event is 3, and one logged before the next request
    // could be 50: asking from 99 again would skip it.
    assert_eq!(next_since(Some(99), &feedback(&[], 3)), Some(3));
}

#[test]
fn asks_again_for_events_that_didnt_end_the_wait() {
    // A wait for the operator still returns these once it ends.
    assert_eq!(next_since(Some(1), &feedback(&[2, 5], 5)), Some(1));
}

#[test]
fn leaves_the_delivered_cursor_to_the_hub() {
    assert_eq!(next_since(None, &feedback(&[], 3)), None);
}
