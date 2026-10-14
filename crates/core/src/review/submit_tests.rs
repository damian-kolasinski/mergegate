use super::*;
use crate::fake::FakeStore;
use crate::feedback::{EventKind, FeedbackStore};

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn request(revision: u32, verdict: Verdict, summary: &str) -> SubmitReview {
    SubmitReview {
        id: id("app#1"),
        revision,
        verdict,
        summary: summary.into(),
    }
}

fn state(store: &FakeStore) -> ItemState {
    store.item(&id("app#1")).unwrap().unwrap().state
}

#[test]
fn requesting_changes_sends_the_item_back_with_an_event() {
    let mut store = FakeStore::with_item(1);
    let review = submit(
        &mut store,
        request(
            1,
            Verdict::ChangesRequested,
            "  Handle the expired token.\n",
        ),
    )
    .unwrap();
    assert_eq!(review.summary, "Handle the expired token.");
    assert_eq!(review.revision, 1);
    assert_eq!(state(&store), ItemState::ChangesRequested);

    let events = store.events(&id("app#1"), 0).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::ReviewSubmitted(review));
}

#[test]
fn a_comment_leaves_the_item_where_it_is() {
    let mut store = FakeStore::with_item(1);
    submit(&mut store, request(1, Verdict::Comment, "Looks close.")).unwrap();
    assert_eq!(state(&store), ItemState::NeedsReview);

    store.set_state(&id("app#1"), ItemState::ChangesRequested);
    submit(&mut store, request(1, Verdict::Comment, "One more thing.")).unwrap();
    assert_eq!(state(&store), ItemState::ChangesRequested);
    assert_eq!(store.events(&id("app#1"), 0).unwrap().len(), 2);
}

#[test]
fn a_review_may_be_on_an_older_revision() {
    let mut store = FakeStore::with_item(2);
    let review = submit(&mut store, request(1, Verdict::Comment, "Written on r1.")).unwrap();
    assert_eq!(review.revision, 1);
}

#[test]
fn refuses_reviews_that_cant_be_sent() {
    let mut store = FakeStore::with_item(1);
    let mut err = |request| submit(&mut store, request).unwrap_err().to_string();
    let other = SubmitReview {
        id: id("app#2"),
        ..request(1, Verdict::Comment, "Hi")
    };
    assert_eq!(err(other), "no item app#2");
    assert_eq!(
        err(request(2, Verdict::Comment, "Hi")),
        "app#1 has no revision 2"
    );
    assert_eq!(
        err(request(0, Verdict::Comment, "Hi")),
        "app#1 has no revision 0"
    );
    assert_eq!(
        err(request(1, Verdict::ChangesRequested, " \n")),
        "a review needs a summary or a comment"
    );
    for (closed, message) in [
        (ItemState::Merging, "app#1 is merging"),
        (ItemState::Merged, "app#1 is closed"),
        (ItemState::Abandoned, "app#1 is closed"),
    ] {
        store.set_state(&id("app#1"), closed);
        let err = submit(&mut store, request(1, Verdict::Comment, "Hi")).unwrap_err();
        assert_eq!(err.to_string(), message);
    }
    assert!(store.reviews().is_empty());
}
