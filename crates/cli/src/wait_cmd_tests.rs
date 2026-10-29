use api::feedback::EventBody;
use api::item::ItemState;

use super::*;

fn feedback(events: &[u64], cursor: u64) -> Feedback {
    Feedback {
        item: "app#1".into(),
        name: "app#1".into(),
        state: ItemState::NeedsReview,
        events: events
            .iter()
            .map(|&cursor| Event {
                cursor,
                item: "app#1".into(),
                name: "app#1".into(),
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

#[test]
fn resumes_after_the_cursor_it_read_after() {
    let stopped = stopped("app#3", Wake::Operator, None, Some(12));
    assert_eq!(stopped.resume, "mg wait app#3 --for operator --since 12");
    assert_eq!(stopped.cursor, Some(12));
}

#[test]
fn resumes_a_plain_wait_with_the_same_command() {
    // The hub keeps the delivered cursor, and nothing was delivered.
    assert_eq!(
        stopped("app#3", Wake::Any, None, None).resume,
        "mg wait app#3"
    );
}

#[test]
fn resumes_with_the_timeout_it_was_given() {
    // A background wait rerun without `--timeout 0` would end after the
    // default and send its agent back for nothing.
    assert_eq!(
        stopped("app#3", Wake::Any, Some(0), None).resume,
        "mg wait app#3 --timeout 0"
    );
}

#[test]
fn reconnecting_gives_up_by_the_deadline_however_often_it_connects() {
    let deadline = Instant::now() + Duration::from_secs(5);
    assert_eq!(Retry::new(Some(deadline)).give_up, deadline);
    let retry = Retry::new(None);
    assert!(retry.give_up <= Instant::now() + RECONNECT);
    assert_eq!(retry.pause, BACKOFF);
}

#[test]
fn a_lost_hub_is_waited_out_and_a_refusal_is_not() {
    assert!(lost(&CliError::new("hub_unavailable", "")));
    assert!(lost(&CliError::new("hub_unresponsive", "")));
    assert!(lost(&CliError::new("tailscale_unavailable", "")));
    assert!(!lost(&CliError::new("hub_incompatible", "")));
    assert!(!lost(&CliError::new("device_denied", "")));
}

#[test]
fn an_error_that_ends_the_wait_says_how_to_resume() {
    let e = resumable(
        CliError::new("hub_unavailable", "lost it").hint("mg hub status"),
        &stopped("app#3", Wake::Any, None, Some(4)),
    );
    assert_eq!(
        e.body.hint.as_deref(),
        Some("mg hub status; once that's fixed, resume with: mg wait app#3 --since 4")
    );
}

#[test]
fn only_claude_code_waits_without_a_timeout() {
    assert_eq!(Host::ClaudeCode.timeout(), 0);
    assert_eq!(Host::Other.timeout(), TIMEOUT);
    assert_eq!(Host::Codex.timeout(), TIMEOUT);
}
