use super::*;
use crate::fake::{FakeGit, FakeGithub, FakeStore, commit};
use crate::github::PullLines;

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
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
        created: 0,
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
    let ingested = ingest(&mut store, &FakeGit::default(), &id(), comments, 5_000).unwrap();
    assert_eq!(
        ingested,
        Ingested {
            new: 1,
            edited: 0,
            withdrawn: 0
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
    ingest(&mut store, &FakeGit::default(), &id(), vec![comment], 0).unwrap();

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
    ingest(&mut store, &FakeGit::default(), &id(), comments, 0).unwrap();

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
    ingest(&mut store, &git, &id(), first.clone(), 1_000).unwrap();
    store.release(&id(), 1).unwrap();

    let again = ingest(&mut store, &git, &id(), first, 2_000).unwrap();
    assert_eq!(again, Ingested::default());

    let edited = vec![issue(1, "alice", "One, edited"), issue(2, "bob", "Two")];
    let ingested = ingest(&mut store, &git, &id(), edited, 3_000).unwrap();
    assert_eq!(
        ingested,
        Ingested {
            new: 0,
            edited: 1,
            withdrawn: 0
        }
    );
    let comments = store.comments(&id()).unwrap();
    assert_eq!(comments[0].body, "One, edited");
    assert!(comments[0].external.as_ref().unwrap().pending);
    assert!(!comments[1].external.as_ref().unwrap().pending);
    assert_eq!(store.external_times(comments[0].id), (3_000, Some(3_000)));
}

#[test]
fn a_held_comment_github_no_longer_has_is_withdrawn() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let first = vec![issue(1, "alice", "One"), issue(2, "bob", "Two")];
    ingest(&mut store, &git, &id(), first, 1_000).unwrap();
    store.release(&id(), 1).unwrap();
    let held = vec![
        issue(1, "alice", "One, edited"),
        issue(2, "bob", "Two"),
        issue(3, "carol", "Three"),
    ];
    ingest(&mut store, &git, &id(), held, 2_000).unwrap();

    let ingested = ingest(&mut store, &git, &id(), Vec::new(), 3_000).unwrap();
    assert_eq!(
        ingested,
        Ingested {
            new: 0,
            edited: 0,
            withdrawn: 2
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
    let again = ingest(&mut store, &git, &id(), Vec::new(), 4_000).unwrap();
    assert_eq!(again, Ingested::default());
}

#[test]
fn a_closed_item_takes_no_more_comments() {
    let mut store = FakeStore::with_item(1);
    store.set_state(&id(), ItemState::Merged);
    let comments = vec![issue(1, "alice", "Late")];
    let ingested = ingest(&mut store, &FakeGit::default(), &id(), comments, 0).unwrap();
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
