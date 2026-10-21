use super::*;
use crate::external::{External, NewExternal, Vote, held_keys, release, vote};
use crate::fake::{FakeStore, remapped};
use crate::github::PullCommentKind;
use crate::item::{ItemId, ItemState};
use crate::review::{Anchor, CommentId, Side, conversation, open_threads, waiting};

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

/// `login`'s comment on r1's line 2 of `path`, which `remapped` drops from
/// `src/gone.rs` in r2 and moves down a line in `src/moved.rs`.
fn on_r1(store: &mut FakeStore, github_id: u64, login: &str, path: &str) -> CommentId {
    let new = NewExternal {
        item: id(),
        revision: 1,
        anchor: Some(Anchor {
            path: path.into(),
            side: Side::New,
            start_line: 2,
            line: 2,
            quote: "B".into(),
        }),
        body: "Rename this.".into(),
        kind: PullCommentKind::ReviewComment,
        github_id,
        login: login.into(),
        bot: login.ends_with("[bot]"),
        url: String::new(),
        path: Some(path.into()),
        summary: false,
        dated: 0,
    };
    store.insert_external(new, 0).unwrap().id
}

fn external(store: &FakeStore, comment: CommentId) -> External {
    store.comment(comment).unwrap().unwrap().external.unwrap()
}

#[test]
fn a_bot_s_comment_on_lines_the_newest_revision_outdated_stops_waiting_for_a_vote() {
    let (mut store, git) = remapped(false);
    let outdated = on_r1(&mut store, 1, "codex[bot]", "src/gone.rs");
    let moved = on_r1(&mut store, 2, "codex[bot]", "src/moved.rs");
    let person = on_r1(&mut store, 3, "alice", "src/gone.rs");
    let voted = on_r1(&mut store, 4, "coderabbitai[bot]", "src/gone.rs");
    vote(&mut store, voted, Vote::Up, 0).unwrap();

    assert_eq!(lapse(&mut store, &git, &id()).unwrap(), 1);

    let lapsed = external(&store, outdated);
    assert!(lapsed.lapsed && !lapsed.pending && !lapsed.awaiting_vote());
    // Its lines are still there to vote on, a person won't say it again, and
    // a vote already cast stands.
    assert!(external(&store, moved).awaiting_vote());
    assert!(external(&store, person).awaiting_vote());
    let voted = external(&store, voted);
    assert!(voted.pending && !voted.lapsed);
    let comments = store.comments(&id()).unwrap();
    assert_eq!(waiting(&comments, &[]).votes, 2);
    // Nothing more to lapse.
    assert_eq!(lapse(&mut store, &git, &id()).unwrap(), 0);
}

#[test]
fn a_lapsed_comment_reaches_the_agent_only_once_voted_up() {
    let (mut store, git) = remapped(false);
    let outdated = on_r1(&mut store, 1, "codex[bot]", "src/gone.rs");
    lapse(&mut store, &git, &id()).unwrap();

    assert_eq!(store.held(&id()).unwrap(), None);
    let read = conversation(&store, &git, &id(), None).unwrap().unwrap();
    assert!(open_threads(&read).is_empty());
    // GitHub deleting it still withdraws it.
    assert!(
        held_keys(&store, &id())
            .unwrap()
            .contains(&(PullCommentKind::ReviewComment, 1))
    );

    vote(&mut store, outdated, Vote::Up, 10).unwrap();
    assert!(!external(&store, outdated).lapsed);
    let event = release(&mut store, &id()).unwrap();
    assert!(event.is_some());
}

#[test]
fn a_closed_item_s_comments_never_lapse() {
    let (mut store, git) = remapped(false);
    on_r1(&mut store, 1, "codex[bot]", "src/gone.rs");
    store.set_state(&id(), ItemState::Abandoned);
    assert_eq!(lapse(&mut store, &git, &id()).unwrap(), 0);
}
