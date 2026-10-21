use super::*;
use crate::external::{Vote, held_keys};
use crate::fake::{FakeGit, FakeGithub, FakeStore, commit};
use crate::github::{PullCommentKind, PullLines};
use crate::review::{Comment, NewReply, ReplyStatus};

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

/// Votes up every comment `app#1` holds and hands them to the agent.
fn send_all(store: &mut FakeStore) {
    for comment in store.comments(&id()).unwrap() {
        if comment.external.is_some_and(|e| e.awaiting_vote()) {
            store.set_vote(comment.id, Vote::Up, 0).unwrap();
        }
    }
    store.release(&id(), 1).unwrap();
}

fn issue(github_id: u64, login: &str, body: &str) -> PullComment {
    PullComment {
        kind: PullCommentKind::IssueComment,
        id: github_id,
        login: login.into(),
        bot: login.ends_with("[bot]"),
        url: format!("https://github.com/o/app/pull/1#issuecomment-{github_id}"),
        body: body.into(),
        lines: None,
        dated: 0,
    }
}

/// On `README.md`, whose r1 and r2 add c1's and c2's lines.
fn inline(github_id: u64, original_commit: &str, line: Option<u32>) -> PullComment {
    PullComment {
        kind: PullCommentKind::ReviewComment,
        lines: Some(PullLines {
            path: "README.md".into(),
            old_side: false,
            original_commit: original_commit.into(),
            original_start_line: None,
            original_line: line,
            commit: original_commit.into(),
            start_line: None,
            line,
        }),
        ..issue(github_id, "codex[bot]", "Typo here.")
    }
}

#[test]
fn an_inline_comment_sits_on_the_revision_it_was_written_on() {
    let mut store = FakeStore::with_item(2);
    let comments = vec![inline(7, &commit(2), Some(2))];
    let ingested = ingest(
        &mut store,
        &FakeGit::default(),
        &id(),
        comments,
        &BTreeSet::new(),
        5_000,
    )
    .unwrap();
    assert_eq!(
        ingested,
        Ingested {
            new: 1,
            edited: 0,
            withdrawn: 0,
            lapsed: 0
        }
    );

    let [comment] = &store.comments(&id()).unwrap()[..] else {
        panic!("one comment");
    };
    assert_eq!(comment.revision, 1);
    let anchor = comment.anchor.as_ref().unwrap();
    assert_eq!(
        (
            anchor.path.as_str(),
            anchor.side,
            anchor.start_line,
            anchor.line
        ),
        ("README.md", Side::New, 2, 2)
    );
    assert_eq!(anchor.quote, commit(2));
    assert!(!comment.is_draft());
    let external = comment.external.as_ref().unwrap();
    assert_eq!(
        (external.kind, external.github_id, external.login.as_str()),
        (PullCommentKind::ReviewComment, 7, "codex[bot]")
    );
    assert!(external.bot && external.pending);
    assert_eq!(store.external_times(comment.id), (5_000, Some(5_000)));
}

#[test]
fn lines_github_carried_place_a_comment_written_on_a_commit_no_revision_has() {
    let mut store = FakeStore::with_item(2);
    let mut comment = inline(7, "f00d", Some(1));
    let lines = comment.lines.as_mut().unwrap();
    lines.commit = commit(3);
    lines.start_line = Some(1);
    lines.line = Some(2);
    ingest(
        &mut store,
        &FakeGit::default(),
        &id(),
        vec![comment],
        &BTreeSet::new(),
        0,
    )
    .unwrap();

    let comment = &store.comments(&id()).unwrap()[0];
    assert_eq!(comment.revision, 2);
    let anchor = comment.anchor.as_ref().unwrap();
    assert_eq!((anchor.start_line, anchor.line), (1, 2));
}

#[test]
fn what_cant_be_placed_sits_on_the_item_as_a_whole() {
    let mut store = FakeStore::with_item(2);
    let comments = vec![
        issue(1, "alice", "Looks good overall."),
        // On the whole file, on lines the diff doesn't show, and on a commit
        // that's no revision's head.
        inline(2, &commit(2), None),
        inline(3, &commit(2), Some(9)),
        inline(4, "f00d", Some(1)),
    ];
    ingest(
        &mut store,
        &FakeGit::default(),
        &id(),
        comments,
        &BTreeSet::new(),
        0,
    )
    .unwrap();

    let comments = store.comments(&id()).unwrap();
    assert_eq!(comments.len(), 4);
    for comment in &comments {
        assert_eq!((comment.revision, &comment.anchor), (2, &None));
    }
    let paths: Vec<_> = comments
        .iter()
        .map(|c| c.external.as_ref().unwrap().path.as_deref())
        .collect();
    assert_eq!(
        paths,
        [
            None,
            Some("README.md"),
            Some("README.md"),
            Some("README.md")
        ]
    );
}

#[test]
fn an_edit_is_held_again_and_the_rest_is_known() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let first = vec![issue(1, "alice", "One"), issue(2, "bob", "Two")];
    ingest(
        &mut store,
        &git,
        &id(),
        first.clone(),
        &BTreeSet::new(),
        1_000,
    )
    .unwrap();
    send_all(&mut store);

    let again = ingest(&mut store, &git, &id(), first, &BTreeSet::new(), 2_000).unwrap();
    assert_eq!(again, Ingested::default());

    let edited = vec![issue(1, "alice", "One, edited"), issue(2, "bob", "Two")];
    let ingested = ingest(&mut store, &git, &id(), edited, &BTreeSet::new(), 3_000).unwrap();
    assert_eq!(
        ingested,
        Ingested {
            new: 0,
            edited: 1,
            withdrawn: 0,
            lapsed: 0
        }
    );
    let comments = store.comments(&id()).unwrap();
    assert_eq!(comments[0].body, "One, edited");
    assert!(comments[0].external.as_ref().unwrap().pending);
    assert!(!comments[1].external.as_ref().unwrap().pending);
    assert_eq!(store.external_times(comments[0].id), (3_000, Some(3_000)));
}

#[test]
fn a_held_comment_github_deleted_is_withdrawn() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let first = vec![issue(1, "alice", "One"), issue(2, "bob", "Two")];
    ingest(&mut store, &git, &id(), first, &BTreeSet::new(), 1_000).unwrap();
    send_all(&mut store);
    let held = vec![
        issue(1, "alice", "One, edited"),
        issue(2, "bob", "Two"),
        issue(3, "carol", "Three"),
    ];
    ingest(&mut store, &git, &id(), held, &BTreeSet::new(), 2_000).unwrap();
    let all: BTreeSet<_> = (1..=3)
        .map(|n| (PullCommentKind::IssueComment, n))
        .collect();

    // Missing from the list alone, they stay held.
    let missed = ingest(&mut store, &git, &id(), Vec::new(), &BTreeSet::new(), 3_000).unwrap();
    assert_eq!(missed, Ingested::default());
    assert_eq!(store.held(&id()).unwrap().map(|h| h.first), Some(2_000));

    let ingested = ingest(&mut store, &git, &id(), Vec::new(), &all, 3_000).unwrap();
    assert_eq!(
        ingested,
        Ingested {
            new: 0,
            edited: 0,
            withdrawn: 2,
            lapsed: 0
        }
    );
    // The one no batch carried is gone; the others read as the agent got
    // them.
    let comments: Vec<_> = store
        .comments(&id())
        .unwrap()
        .into_iter()
        .map(|c| (c.body, c.external.unwrap().pending))
        .collect();
    assert_eq!(comments, [("One".into(), false), ("Two".into(), false)]);
    assert_eq!(store.held(&id()).unwrap(), None);
    let again = ingest(&mut store, &git, &id(), Vec::new(), &all, 4_000).unwrap();
    assert_eq!(again, Ingested::default());
}

const WALKTHROUGH: &str =
    "<!-- This is an auto-generated comment: summarize by coderabbit.ai -->\n## Walkthrough";

#[test]
fn a_bots_summary_is_kept_but_never_held() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let summary = issue(1, "coderabbitai[bot]", WALKTHROUGH);
    let ingested = ingest(
        &mut store,
        &git,
        &id(),
        vec![summary.clone()],
        &BTreeSet::new(),
        1_000,
    )
    .unwrap();
    assert_eq!(ingested.new, 1);
    let external = store.comments(&id()).unwrap()[0].external.clone().unwrap();
    assert!(external.summary && !external.pending && !external.awaiting_vote());
    assert_eq!(store.held(&id()).unwrap(), None);

    // The bot rewrites it on every review; it still asks nothing.
    let rewritten = PullComment {
        body: format!("{WALKTHROUGH}\n\nReviewed again."),
        ..summary
    };
    let ingested = ingest(
        &mut store,
        &git,
        &id(),
        vec![rewritten],
        &BTreeSet::new(),
        2_000,
    )
    .unwrap();
    assert_eq!(ingested.edited, 1);
    let external = store.comments(&id()).unwrap()[0].external.clone().unwrap();
    assert!(external.summary && !external.pending);
    assert_eq!(store.held(&id()).unwrap(), None);
}

#[test]
fn a_summary_held_before_summaries_were_told_apart_stops_waiting() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let summary = issue(1, "coderabbitai[bot]", WALKTHROUGH);
    let held = NewExternal {
        item: id(),
        revision: 1,
        anchor: None,
        body: summary.body.clone(),
        kind: summary.kind,
        github_id: summary.id,
        login: summary.login.clone(),
        bot: true,
        url: summary.url.clone(),
        path: None,
        summary: false,
        dated: 0,
    };
    store.insert_external(held, 500).unwrap();
    assert!(store.held(&id()).unwrap().unwrap().awaiting);

    let ingested = ingest(
        &mut store,
        &git,
        &id(),
        vec![summary],
        &BTreeSet::new(),
        1_000,
    )
    .unwrap();
    assert_eq!(ingested.edited, 1);
    let external = store.comments(&id()).unwrap()[0].external.clone().unwrap();
    assert!(external.summary && !external.pending);
    assert_eq!(store.held(&id()).unwrap(), None);
}

#[test]
fn a_summary_the_agent_got_before_summaries_were_told_apart_stays_its_own() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let summary = issue(1, "coderabbitai[bot]", WALKTHROUGH);
    let sent = NewExternal {
        item: id(),
        revision: 1,
        anchor: None,
        body: summary.body.clone(),
        kind: summary.kind,
        github_id: summary.id,
        login: summary.login.clone(),
        bot: true,
        url: summary.url.clone(),
        path: None,
        summary: false,
        dated: 0,
    };
    store.insert_external(sent, 500).unwrap();
    send_all(&mut store);

    let ingested = ingest(
        &mut store,
        &git,
        &id(),
        vec![summary],
        &BTreeSet::new(),
        1_000,
    )
    .unwrap();
    assert_eq!(ingested, Ingested::default());
    let external = store.comments(&id()).unwrap()[0].external.clone().unwrap();
    assert!(!external.summary && !external.pending);
    assert_eq!(held_keys(&store, &id()).unwrap(), BTreeSet::new());
}

#[test]
fn a_closed_item_takes_no_more_comments() {
    let mut store = FakeStore::with_item(1);
    store.set_state(&id(), ItemState::Merged);
    let comments = vec![issue(1, "alice", "Late")];
    let ingested = ingest(
        &mut store,
        &FakeGit::default(),
        &id(),
        comments,
        &BTreeSet::new(),
        0,
    )
    .unwrap();
    assert_eq!(ingested, Ingested::default());
    assert!(store.comments(&id()).unwrap().is_empty());
}

#[test]
fn the_operator_s_own_and_empty_comments_are_not_fetched() {
    let github = FakeGithub::default();
    for comment in [
        issue(1, "alice", "Hi"),
        issue(2, "Operator", "My own"),
        issue(3, "bob", "  \n"),
    ] {
        github.comment("o/app", 1, comment);
    }
    let repo = GithubRepo::parse("o/app").unwrap();
    let fetched = fetch(&github, &repo, 1, false).unwrap();
    assert_eq!(fetched, [issue(1, "alice", "Hi")]);
}

/// Brings `first` in, hands it to the agent and, when `answer`, has the
/// agent reply to it; then brings in `edited`, the same comment edited, and
/// returns it as the hub has it.
fn edited_after_the_agent_got_it(first: PullComment, edited: &str, answer: bool) -> Comment {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let none = BTreeSet::new();
    ingest(&mut store, &git, &id(), vec![first.clone()], &none, 1_000).unwrap();
    send_all(&mut store);
    let comment = store.comments(&id()).unwrap()[0].id;
    if answer {
        let reply = NewReply {
            comment,
            item: id(),
            author: Author::Agent,
            status: Some(ReplyStatus::Addressed),
            body: String::new(),
            revision: 1,
        };
        store.insert_reply(reply).unwrap();
    }
    let edit = PullComment {
        body: edited.into(),
        dated: 2,
        ..first
    };
    let ingested = ingest(&mut store, &git, &id(), vec![edit], &none, 2_000).unwrap();
    assert_eq!(ingested.edited, 1);
    store.comments(&id()).unwrap().remove(0)
}

#[test]
fn a_bots_edit_to_a_comment_the_agent_answered_stays_with_the_agent() {
    let bot = issue(1, "coderabbitai[bot]", "Typo here.");
    let comment =
        edited_after_the_agent_got_it(bot, "Typo here.\n\n✅ Addressed in commit abc123", true);
    assert_eq!(comment.body, "Typo here.\n\n✅ Addressed in commit abc123");
    let external = comment.external.unwrap();
    assert!(!external.pending, "redelivered");
    assert_eq!(external.vote, Some(Vote::Up));
}

#[test]
fn a_bots_edit_to_a_comment_the_agent_left_unanswered_is_held_again() {
    let bot = issue(1, "coderabbitai[bot]", "Typo here.");
    let comment = edited_after_the_agent_got_it(bot, "Typo here, and there.", false);
    assert!(comment.external.unwrap().awaiting_vote());
}

#[test]
fn a_persons_edit_to_a_comment_the_agent_answered_is_held_again() {
    // It can ask for more than the agent answered.
    let person = issue(1, "alice", "Rename it.");
    let comment = edited_after_the_agent_got_it(person, "Rename it, and the test.", true);
    assert!(comment.external.unwrap().awaiting_vote());
}

#[test]
fn a_bots_placeholder_the_agent_got_that_became_its_summary_stays_with_the_agent() {
    let placeholder = issue(1, "coderabbitai[bot]", "Currently processing new changes.");
    let comment = edited_after_the_agent_got_it(placeholder, WALKTHROUGH, false);
    let external = comment.external.unwrap();
    assert!(!external.pending && !external.summary);
}
