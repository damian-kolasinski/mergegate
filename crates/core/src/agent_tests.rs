use super::*;
use crate::fake::{FakeGit, FakeStore};
use crate::feedback::acknowledge;
use crate::item::ItemStore;
use crate::review::{SubmitReview, Verdict, submit};

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn agent(worktree: Option<&str>, wake: Option<&str>) -> Agent {
    Agent {
        worktree: worktree.map(Into::into),
        wake: wake.map(Into::into),
    }
}

#[test]
fn a_submit_keeps_what_it_does_not_carry() {
    // (stored, worktree sent, wake sent, stored after)
    let cases = [
        (agent(None, None), Some("/w"), None, agent(Some("/w"), None)),
        (
            agent(None, None),
            None,
            Some("resume"),
            agent(None, Some("resume")),
        ),
        (
            agent(Some("/w"), Some("old")),
            None,
            None,
            agent(Some("/w"), Some("old")),
        ),
        (
            agent(Some("/w"), Some("old")),
            Some("/v"),
            None,
            agent(Some("/v"), Some("old")),
        ),
        (
            agent(Some("/w"), Some("old")),
            None,
            Some("new"),
            agent(Some("/w"), Some("new")),
        ),
        (
            agent(Some("/w"), Some("old")),
            None,
            Some(""),
            agent(Some("/w"), None),
        ),
        (
            agent(Some("/w"), Some("old")),
            None,
            Some("  "),
            agent(Some("/w"), None),
        ),
        (
            agent(None, None),
            None,
            Some(" resume \n"),
            agent(None, Some("resume")),
        ),
    ];
    for (stored, worktree, wake, after) in cases {
        let mut store = FakeStore::with_item(1);
        store.set_agent(&id(), &stored).unwrap();
        let sent = (worktree, wake);
        record(
            &mut store,
            &id(),
            worktree.map(Into::into),
            wake.map(Into::into),
        )
        .unwrap();
        assert_eq!(store.agent(&id()).unwrap(), after, "{stored:?} {sent:?}");
    }
}

#[test]
fn the_wake_command_names_the_item() {
    assert_eq!(
        wake_command("harness resume --item {item} # {item}", &id()),
        "harness resume --item app#1 # app#1"
    );
    assert_eq!(wake_command("resume", &id()), "resume");
}

fn item(store: &FakeStore) -> Item {
    store.item(&id()).unwrap().unwrap()
}

fn request_changes(store: &mut FakeStore) -> u64 {
    let request = SubmitReview {
        id: id(),
        revision: 1,
        verdict: Verdict::ChangesRequested,
        summary: "Fix it".into(),
    };
    submit(store, &FakeGit::default(), request).unwrap();
    store.last_cursor(&id()).unwrap()
}

#[test]
fn a_read_in_flight_is_an_agent_waiting() {
    let store = FakeStore::with_item(1);
    let item = item(&store);
    assert_eq!(
        presence(&store, &item, true, false).unwrap(),
        Some(Presence::Waiting)
    );
    assert_eq!(
        presence(&store, &item, false, true).unwrap(),
        Some(Presence::Wakes)
    );
    assert_eq!(
        presence(&store, &item, false, false).unwrap(),
        Some(Presence::Away)
    );
}

#[test]
fn an_agent_that_read_the_changes_asked_for_is_working() {
    let mut store = FakeStore::with_item(1);
    let cursor = request_changes(&mut store);
    // Unread, the changes wait for whatever picks them up.
    assert_eq!(
        presence(&store, &item(&store), false, false).unwrap(),
        Some(Presence::Away)
    );
    assert_eq!(
        presence(&store, &item(&store), false, true).unwrap(),
        Some(Presence::Wakes)
    );
    acknowledge(&mut store, &id(), cursor, None).unwrap();
    assert_eq!(
        presence(&store, &item(&store), false, false).unwrap(),
        Some(Presence::Working)
    );
}

#[test]
fn a_closed_item_has_no_presence() {
    let mut store = FakeStore::with_item(1);
    store.set_state(&id(), ItemState::Merged);
    assert_eq!(presence(&store, &item(&store), true, true).unwrap(), None);
}
