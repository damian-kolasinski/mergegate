use super::*;
use crate::external::NewExternal;
use crate::fake::{FakeGit, FakeStore};
use crate::feedback::{acknowledge, feedback};
use crate::github::PullCommentKind;
use crate::review::{SubmitReview, Verdict, submit};

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn hold(store: &mut FakeStore, github_id: u64) {
    let new = NewExternal {
        item: id(),
        revision: 1,
        anchor: None,
        body: "Needs a test.".into(),
        kind: PullCommentKind::IssueComment,
        github_id,
        login: "alice".into(),
        bot: false,
        url: String::new(),
        path: None,
        dated: 0,
    };
    store.insert_external(new, 0).unwrap();
}

fn review(store: &mut FakeStore) -> u64 {
    let request = SubmitReview {
        id: id(),
        revision: 1,
        verdict: Verdict::ChangesRequested,
        summary: "Again.".into(),
    };
    submit(store, &FakeGit::default(), request).unwrap();
    store.last_cursor(&id()).unwrap()
}

#[test]
fn a_read_on_from_the_delivered_cursor_takes_the_held_comments() {
    let mut store = FakeStore::with_item(1);
    let first = review(&mut store);
    acknowledge(&mut store, &id(), first, None).unwrap();
    hold(&mut store, 1);
    review(&mut store);

    for since in [None, Some(first)] {
        let handed = feedback(&store, &id(), since).unwrap().events;
        assert!(release_on_read(&mut store, &id(), since, &handed).unwrap());
        hold(&mut store, since.map_or(2, |_| 3));
    }
}

#[test]
fn a_look_back_and_an_empty_read_leave_them_held() {
    let mut store = FakeStore::with_item(1);
    let first = review(&mut store);
    acknowledge(&mut store, &id(), first, None).unwrap();
    hold(&mut store, 1);

    let handed = feedback(&store, &id(), Some(0)).unwrap().events;
    assert!(!release_on_read(&mut store, &id(), Some(0), &handed).unwrap());
    assert!(!release_on_read(&mut store, &id(), None, &[]).unwrap());
    assert!(store.held(&id()).unwrap().is_some());
}
