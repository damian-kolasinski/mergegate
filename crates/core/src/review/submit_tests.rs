use super::*;
use crate::fake::{FakeGit, FakeStore};
use crate::feedback::{EventKind, FeedbackStore};
use crate::review::{Side, WriteDraft, write_draft};

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
        &FakeGit::default(),
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
fn a_review_sends_every_draft_on_the_item() {
    let mut store = FakeStore::with_item(2);
    let git = FakeGit::default();
    for revision in [1, 2] {
        let draft = WriteDraft {
            id: id("app#1"),
            revision,
            path: "README.md".into(),
            side: Side::New,
            start_line: 1,
            line: 1,
            body: format!("On r{revision}."),
        };
        write_draft(&mut store, &git, draft).unwrap();
    }

    let review = submit(
        &mut store,
        &FakeGit::default(),
        request(2, Verdict::ChangesRequested, ""),
    )
    .unwrap();
    let sent: Vec<_> = review
        .comments
        .iter()
        .map(|c| (c.revision, c.body.as_str(), c.review))
        .collect();
    // The draft from r1 goes on r2, which didn't change its line.
    assert_eq!(sent, [(2, "On r1.", Some(1)), (2, "On r2.", Some(1))]);
    let events = store.events(&id("app#1"), 0).unwrap();
    assert_eq!(events[0].kind, EventKind::ReviewSubmitted(review));

    // They went out with the first review.
    let next = submit(
        &mut store,
        &FakeGit::default(),
        request(2, Verdict::Comment, "And?"),
    )
    .unwrap();
    assert_eq!(next.comments, []);
    assert_eq!(
        submit(
            &mut store,
            &FakeGit::default(),
            request(2, Verdict::Comment, "")
        )
        .unwrap_err()
        .to_string(),
        "a review needs a summary, a comment or a reply"
    );
}

#[test]
fn a_comment_leaves_the_item_where_it_is() {
    let mut store = FakeStore::with_item(1);
    submit(
        &mut store,
        &FakeGit::default(),
        request(1, Verdict::Comment, "Looks close."),
    )
    .unwrap();
    assert_eq!(state(&store), ItemState::NeedsReview);

    store.set_state(&id("app#1"), ItemState::ChangesRequested);
    submit(
        &mut store,
        &FakeGit::default(),
        request(1, Verdict::Comment, "One more thing."),
    )
    .unwrap();
    assert_eq!(state(&store), ItemState::ChangesRequested);
    assert_eq!(store.events(&id("app#1"), 0).unwrap().len(), 2);
}

#[test]
fn a_review_may_be_on_an_older_revision() {
    let mut store = FakeStore::with_item(2);
    let review = submit(
        &mut store,
        &FakeGit::default(),
        request(1, Verdict::Comment, "Written on r1."),
    )
    .unwrap();
    assert_eq!(review.revision, 1);
}

#[test]
fn changes_are_requested_on_the_newest_revision_only() {
    let mut store = FakeStore::with_item(2);
    let err = submit(
        &mut store,
        &FakeGit::default(),
        request(1, Verdict::ChangesRequested, "Fix r1."),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "app#1 r2 came after r1; request changes on r2"
    );
    assert_eq!(state(&store), ItemState::NeedsReview);
    assert!(store.reviews().is_empty());
}

#[test]
fn refuses_reviews_that_cant_be_sent() {
    let mut store = FakeStore::with_item(1);
    let mut err = |request| {
        submit(&mut store, &FakeGit::default(), request)
            .unwrap_err()
            .to_string()
    };
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
        "a review needs a summary, a comment or a reply"
    );
    for (closed, message) in [
        (ItemState::Merging, "app#1 is merging"),
        (ItemState::Merged, "app#1 is closed"),
        (ItemState::Abandoned, "app#1 is closed"),
    ] {
        store.set_state(&id("app#1"), closed);
        let err = submit(
            &mut store,
            &FakeGit::default(),
            request(1, Verdict::Comment, "Hi"),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), message);
    }
    assert!(store.reviews().is_empty());
}
