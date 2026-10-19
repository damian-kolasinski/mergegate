use super::*;
use crate::external::{NewExternal, Timing, release, release_due};
use crate::fake::{FakeGit, FakeStore};
use crate::feedback::{Event, EventKind};
use crate::github::PullCommentKind;
use crate::item::ItemId;
use crate::review::{
    ReplyStatus, Side, WriteDraft, WriteReply, conversation, open_threads, reply, waiting,
    write_draft,
};

const TIMING: Timing = Timing {
    quiet: 60,
    cap: 1_000,
};

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

/// A comment on `app#1` the hub takes in at `now`.
fn comment(store: &mut FakeStore, github_id: u64, login: &str, now: u64) -> CommentId {
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
        summary: false,
        dated: now,
    };
    store.insert_external(new, now).unwrap().id
}

fn carried(event: Option<Event>) -> Vec<CommentId> {
    let Some(event) = event else {
        return Vec::new();
    };
    let EventKind::ExternalComments(batch) = event.kind else {
        panic!("{event:?}");
    };
    batch.comments.iter().map(|d| d.comment.id).collect()
}

#[test]
fn the_batch_waits_for_every_vote_and_carries_only_those_voted_up() {
    let mut store = FakeStore::with_item(1);
    let person = comment(&mut store, 1, "ana", 10);
    let useful = comment(&mut store, 2, "codex[bot]", 10);
    let noise = comment(&mut store, 3, "codex[bot]", 10);

    // Quiet long past the cap, but the operator hasn't voted.
    assert!(store.held(&id()).unwrap().unwrap().awaiting);
    assert_eq!(release_due(&mut store, &id(), 5_000, TIMING).unwrap(), None);

    vote(&mut store, person, Vote::Up, 5_000).unwrap();
    vote(&mut store, useful, Vote::Up, 5_000).unwrap();
    assert_eq!(release_due(&mut store, &id(), 5_000, TIMING).unwrap(), None);
    vote(&mut store, noise, Vote::Down, 5_000).unwrap();
    let event = release_due(&mut store, &id(), 5_000, TIMING).unwrap();
    assert_eq!(carried(event), [person, useful]);

    let noise = store.comment(noise).unwrap().unwrap().external.unwrap();
    assert!(noise.dismissed() && !noise.pending);
    assert_eq!(store.held(&id()).unwrap(), None);
}

#[test]
fn a_person_s_comment_waits_for_a_vote_like_a_bot_s() {
    let mut store = FakeStore::with_item(1);
    let person = comment(&mut store, 1, "ana", 10);
    assert!(store.held(&id()).unwrap().unwrap().awaiting);
    vote(&mut store, person, Vote::Down, 20).unwrap();
    assert_eq!(release(&mut store, &id()).unwrap(), None);
    assert_eq!(store.held(&id()).unwrap(), None);
}

#[test]
fn a_read_or_sync_takes_what_was_voted_up_and_leaves_what_awaits_a_vote() {
    let mut store = FakeStore::with_item(1);
    let up = comment(&mut store, 1, "ana", 10);
    let unvoted = comment(&mut store, 2, "codex[bot]", 10);
    vote(&mut store, up, Vote::Up, 20).unwrap();
    assert_eq!(carried(release(&mut store, &id()).unwrap()), [up]);
    assert_eq!(release(&mut store, &id()).unwrap(), None);
    assert!(
        store
            .comment(unvoted)
            .unwrap()
            .unwrap()
            .external
            .unwrap()
            .awaiting_vote()
    );

    vote(&mut store, unvoted, Vote::Up, 30).unwrap();
    assert_eq!(carried(release(&mut store, &id()).unwrap()), [unvoted]);
}

#[test]
fn a_comment_voted_down_is_no_thread_of_the_agent_s() {
    let mut store = FakeStore::with_item(1);
    let noise = comment(&mut store, 1, "codex[bot]", 10);
    vote(&mut store, noise, Vote::Down, 20).unwrap();
    let threads = open_threads(
        &conversation(&store, &FakeGit::default(), &id(), None)
            .unwrap()
            .unwrap(),
    );
    assert!(threads.is_empty());
}

#[test]
fn a_vote_changes_until_a_batch_carries_the_comment() {
    let mut store = FakeStore::with_item(1);
    let bot = comment(&mut store, 1, "codex[bot]", 10);
    vote(&mut store, bot, Vote::Down, 20).unwrap();
    let voted = vote(&mut store, bot, Vote::Up, 30).unwrap();
    let external = voted.external.unwrap();
    assert_eq!((external.vote, external.pending), (Some(Vote::Up), true));

    release(&mut store, &id()).unwrap();
    assert!(matches!(
        vote(&mut store, bot, Vote::Down, 40),
        Err(ExternalError::AlreadySent(c)) if c == bot
    ));
}

#[test]
fn only_a_comment_from_github_takes_a_vote() {
    let mut store = FakeStore::with_item(1);
    let draft = WriteDraft {
        id: id(),
        revision: 1,
        path: "README.md".into(),
        side: Side::New,
        start_line: 1,
        line: 1,
        body: "Why?".into(),
    };
    let own = write_draft(&mut store, &FakeGit::default(), draft).unwrap();
    assert!(matches!(
        vote(&mut store, own.id, Vote::Up, 20),
        Err(ExternalError::NoComment(c)) if c == own.id
    ));
}

#[test]
fn an_edit_asks_for_a_new_vote_up_but_keeps_one_down() {
    let mut store = FakeStore::with_item(1);
    let up = comment(&mut store, 1, "codex[bot]", 10);
    let down = comment(&mut store, 2, "ana", 10);
    vote(&mut store, up, Vote::Up, 20).unwrap();
    vote(&mut store, down, Vote::Down, 20).unwrap();
    store.edit_external(up, "Also this", 30, false, 30).unwrap();
    store
        .edit_external(down, "Still noise", 30, false, 30)
        .unwrap();

    let up = store.comment(up).unwrap().unwrap().external.unwrap();
    assert!(up.awaiting_vote());
    let down = store.comment(down).unwrap().unwrap().external.unwrap();
    assert!(down.dismissed() && !down.pending);
}

#[test]
fn a_comment_awaiting_a_vote_waits_on_the_operator() {
    let mut store = FakeStore::with_item(1);
    let bot = comment(&mut store, 1, "codex[bot]", 10);
    let person = comment(&mut store, 2, "ana", 10);
    vote(&mut store, person, Vote::Up, 10).unwrap();
    release(&mut store, &id()).unwrap();
    // The agent's rejection of a comment it got waits on the operator too.
    let answer = WriteReply {
        comment: person,
        status: ReplyStatus::Rejected,
        body: "No.".into(),
    };
    reply(&mut store, answer).unwrap();
    let owed = |store: &FakeStore| {
        waiting(
            &store.comments(&id()).unwrap(),
            &store.replies(&id()).unwrap(),
        )
    };
    assert_eq!((owed(&store).votes, owed(&store).rejections), (1, 1));

    vote(&mut store, bot, Vote::Down, 20).unwrap();
    assert_eq!(owed(&store).votes, 0);
}

#[test]
fn a_bots_summary_takes_no_vote_and_is_no_thread() {
    let mut store = FakeStore::with_item(1);
    let new = NewExternal {
        item: id(),
        revision: 1,
        anchor: None,
        body: "## Pull request overview".into(),
        kind: PullCommentKind::Review,
        github_id: 1,
        login: "copilot-pull-request-reviewer[bot]".into(),
        bot: true,
        url: String::new(),
        path: None,
        summary: true,
        dated: 10,
    };
    let summary = store.insert_external(new, 10).unwrap().id;

    assert!(matches!(
        vote(&mut store, summary, Vote::Up, 20),
        Err(ExternalError::Summary(id)) if id == summary
    ));
    assert_eq!(store.held(&id()).unwrap(), None);
    let comments = store.comments(&id()).unwrap();
    assert_eq!(waiting(&comments, &[]).votes, 0);
    let threads = open_threads(
        &conversation(&store, &FakeGit::default(), &id(), None)
            .unwrap()
            .unwrap(),
    );
    assert!(threads.is_empty());
}
