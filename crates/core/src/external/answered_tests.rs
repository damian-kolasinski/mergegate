use super::*;
use crate::external::{NewExternal, vote};
use crate::fake::FakeStore;
use crate::feedback::EventKind;
use crate::github::PullCommentKind;
use crate::review::{Author, CommentId};

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn comment(store: &mut FakeStore, github_id: u64, summary: bool) -> CommentId {
    let new = NewExternal {
        item: id(),
        revision: 1,
        anchor: None,
        body: format!("Finding {github_id}"),
        kind: PullCommentKind::IssueComment,
        github_id,
        login: "codex[bot]".into(),
        bot: true,
        url: String::new(),
        path: None,
        summary,
        dated: 10,
    };
    store.insert_external(new, 10).unwrap().id
}

/// The operator's reply on `comment`, as a review sends it.
fn answer(comment: CommentId) -> Reply {
    Reply {
        id: 1,
        comment,
        item: id(),
        author: Author::Operator,
        status: None,
        body: "Check if this is valid.".into(),
        revision: 1,
        review: Some(1),
        overrides: false,
        created_at: String::new(),
    }
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
fn a_reply_sends_the_comment_it_answers_while_others_await_votes() {
    let mut store = FakeStore::with_item(1);
    let answered = comment(&mut store, 1, false);
    let unvoted = comment(&mut store, 2, false);
    let event = send_answered(&mut store, &id(), &[answer(answered)], 20).unwrap();
    assert_eq!(carried(event), [answered]);
    let unvoted = store.comment(unvoted).unwrap().unwrap().external.unwrap();
    assert!(unvoted.awaiting_vote(), "{unvoted:?}");
}

#[test]
fn a_reply_sends_a_comment_voted_down() {
    let mut store = FakeStore::with_item(1);
    let answered = comment(&mut store, 1, false);
    vote(&mut store, answered, Vote::Down, 15).unwrap();
    let event = send_answered(&mut store, &id(), &[answer(answered)], 20).unwrap();
    assert_eq!(carried(event), [answered]);
}

#[test]
fn a_reply_on_a_comment_the_agent_has_or_a_summary_sends_nothing() {
    let mut store = FakeStore::with_item(1);
    let sent = comment(&mut store, 1, false);
    vote(&mut store, sent, Vote::Up, 15).unwrap();
    release(&mut store, &id()).unwrap();
    let summary = comment(&mut store, 2, true);
    let replies = [answer(sent), answer(summary)];
    assert_eq!(
        send_answered(&mut store, &id(), &replies, 20).unwrap(),
        None
    );
}
