use super::*;
use crate::external::{ExternalStore, NewExternal, Vote};
use crate::fake::FakeStore;
use crate::feedback::EventKind;
use crate::github::PullCommentKind;
use crate::project::{Project, ProjectName};
use crate::review::ReviewStore;

const TIMING: Timing = Timing {
    quiet: 60,
    cap: 1_000,
};

fn held(first: u64, last: u64, heard: &[&str]) -> Held {
    Held {
        awaiting: false,
        first,
        last,
        pushed: 0,
        heard: heard.iter().map(|&s| s.into()).collect(),
    }
}

#[test]
fn a_batch_waits_for_quiet_and_the_expected_bots() {
    let bots = ["codex".to_owned(), "claude".to_owned()];
    let cases: &[(&str, Held, &[String], u64, bool)] = &[
        ("nobody expected, quiet", held(10, 10, &[]), &[], 70, true),
        (
            "nobody expected, not quiet",
            held(10, 50, &[]),
            &[],
            70,
            false,
        ),
        (
            "one bot missing",
            held(10, 10, &["codex[bot]"]),
            &bots,
            500,
            false,
        ),
        (
            "every bot heard",
            held(10, 10, &["Codex[bot]", "claude"]),
            &bots,
            70,
            true,
        ),
        (
            "bots heard, not quiet",
            held(10, 50, &["codex", "claude"]),
            &bots,
            70,
            false,
        ),
        (
            "a bot missing past the cap since the push",
            held(990, 990, &[]),
            &bots,
            1_050,
            true,
        ),
        (
            "past the cap since the first, never quiet",
            held(10, 1_005, &[]),
            &bots,
            1_010,
            true,
        ),
    ];
    for (name, held, expected, now, due_now) in cases {
        assert_eq!(due(held, expected, *now, TIMING), *due_now, "{name}");
    }
}

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn comment(store: &mut FakeStore, github_id: u64, login: &str, now: u64) {
    posted(store, github_id, login, now, now);
}

/// A comment GitHub dates `dated` that the hub sees at `now`, which the
/// operator votes up at once, so only the timing holds it.
fn posted(store: &mut FakeStore, github_id: u64, login: &str, dated: u64, now: u64) {
    let new = NewExternal {
        item: id(),
        revision: 1,
        anchor: None,
        body: format!("From {login}"),
        kind: PullCommentKind::IssueComment,
        github_id,
        login: login.into(),
        bot: login.ends_with("[bot]"),
        url: String::new(),
        path: None,
        dated,
    };
    let id = store.insert_external(new, now).unwrap().id;
    store.set_vote(id, Vote::Up, now).unwrap();
}

fn expecting(store: &mut FakeStore, bots: &[&str]) {
    let name = ProjectName::parse("app").unwrap();
    let project = Project {
        bots: bots.iter().map(|&b| b.into()).collect(),
        ..store.project(&name).unwrap().unwrap()
    };
    store.update_project(&project).unwrap();
}

#[test]
fn held_comments_go_once_the_project_s_bots_have_spoken() {
    let mut store = FakeStore::with_item(1);
    expecting(&mut store, &["codex"]);
    let pushed = FakeStore::millis(&store.item(&id()).unwrap().unwrap().revision.submitted_at);
    comment(&mut store, 1, "alice", pushed + 10);
    assert_eq!(
        release_due(&mut store, &id(), pushed + 500, TIMING).unwrap(),
        None
    );

    comment(&mut store, 2, "codex[bot]", pushed + 600);
    assert_eq!(
        release_due(&mut store, &id(), pushed + 650, TIMING).unwrap(),
        None
    );
    let event = release_due(&mut store, &id(), pushed + 660, TIMING)
        .unwrap()
        .unwrap();
    let EventKind::ExternalComments(batch) = event.kind else {
        panic!("{event:?}");
    };
    assert_eq!(batch.revision, 1);
    let bodies: Vec<_> = batch
        .comments
        .iter()
        .map(|d| d.comment.body.as_str())
        .collect();
    assert_eq!(bodies, ["From alice", "From codex[bot]"]);
    assert!(batch.comments.iter().all(|d| !d.edited));
    assert_eq!(store.held(&id()).unwrap(), None);
}

#[test]
fn a_bot_counts_by_when_github_dates_its_comment_not_when_the_hub_saw_it() {
    let mut store = FakeStore::with_item(1);
    expecting(&mut store, &["codex"]);
    let pushed = FakeStore::millis(&store.item(&id()).unwrap().unwrap().revision.submitted_at);
    // Its review of the revision before reached the hub after the push.
    posted(&mut store, 1, "codex[bot]", pushed - 1_000, pushed + 10);
    assert_eq!(
        release_due(&mut store, &id(), pushed + 500, TIMING).unwrap(),
        None
    );

    posted(&mut store, 2, "codex[bot]", pushed, pushed + 600);
    assert!(
        release_due(&mut store, &id(), pushed + 660, TIMING)
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_bot_editing_an_older_comment_after_the_push_counts() {
    let mut store = FakeStore::with_item(1);
    expecting(&mut store, &["codex"]);
    let pushed = FakeStore::millis(&store.item(&id()).unwrap().unwrap().revision.submitted_at);
    // A bot that keeps one summary comment edits it when it reviews a push.
    posted(&mut store, 1, "codex[bot]", pushed - 1_000, pushed + 10);
    let summary = store.comments(&id()).unwrap()[0].id;
    // An edit GitHub dates before the push, though seen after it, doesn't.
    store
        .edit_external(summary, "Reviewing", pushed - 500, pushed + 20)
        .unwrap();
    store.set_vote(summary, Vote::Up, pushed + 20).unwrap();
    assert_eq!(
        release_due(&mut store, &id(), pushed + 500, TIMING).unwrap(),
        None
    );

    store
        .edit_external(summary, "Reviewed", pushed + 600, pushed + 600)
        .unwrap();
    store.set_vote(summary, Vote::Up, pushed + 600).unwrap();
    assert!(
        release_due(&mut store, &id(), pushed + 660, TIMING)
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_comment_carried_again_is_edited() {
    let mut store = FakeStore::with_item(1);
    comment(&mut store, 1, "alice", 0);
    release(&mut store, &id()).unwrap().unwrap();
    let carried = store.comments(&id()).unwrap()[0].id;
    store
        .edit_external(carried, "Changed my mind", 5, 5)
        .unwrap();
    store.set_vote(carried, Vote::Up, 5).unwrap();

    let event = release(&mut store, &id()).unwrap().unwrap();
    let EventKind::ExternalComments(batch) = event.kind else {
        panic!("{event:?}");
    };
    assert_eq!(batch.comments.len(), 1);
    assert!(batch.comments[0].edited);
    assert_eq!(batch.comments[0].comment.body, "Changed my mind");
    assert_eq!(release(&mut store, &id()).unwrap(), None);
}

#[test]
fn a_merging_item_holds_its_comments() {
    let mut store = FakeStore::with_item(1);
    comment(&mut store, 1, "alice", 0);
    store.set_state(&id(), ItemState::Merging);
    assert_eq!(release(&mut store, &id()).unwrap(), None);
    assert!(store.held(&id()).unwrap().is_some());
}
