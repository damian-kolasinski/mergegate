use api::feedback::{Event, EventBody, Source};

use super::*;

fn feedback(cursors: &[u64]) -> Feedback {
    Feedback {
        item: "app#1".into(),
        name: "app#7".into(),
        state: ItemState::NeedsReview,
        events: cursors
            .iter()
            .map(|&cursor| Event {
                cursor,
                item: "app#1".into(),
                name: "app#7".into(),
                revision: 1,
                source: Source::Operator,
                body: EventBody::MergeCancelled {},
            })
            .collect(),
        cursor: cursors.last().copied().unwrap_or(0),
    }
}

fn printed(outcome: Outcome) -> String {
    match outcome {
        Outcome::Done {
            text, exit, then, ..
        } => {
            assert_eq!(exit, 0);
            // Nothing runs after printing: no acknowledgement.
            assert!(then.is_none());
            text
        }
        _ => panic!("not done"),
    }
}

#[test]
fn names_the_events_and_the_command_that_delivers_them() {
    assert_eq!(
        printed(arrived(&feedback(&[3, 5]), None)),
        "app#7 has new events: [3] merge.cancelled, [5] merge.cancelled. Nothing is marked \
         delivered; read and act on them with: mg feedback app#7"
    );
}

#[test]
fn reads_after_the_cursor_the_wait_read_after() {
    assert!(printed(arrived(&feedback(&[5]), Some(4))).ends_with("mg feedback app#7 --since 4"));
}
