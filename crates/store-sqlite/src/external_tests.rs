use std::time::{SystemTime, UNIX_EPOCH};

use mergegate_core::feedback::FeedbackStore;
use mergegate_core::github::PullCommentKind;
use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};
use mergegate_core::review::{Anchor, Author, NewReview, ReplyStatus, ReviewStore, Side, Verdict};

use super::*;

/// `app#1` with one revision.
fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    let name = ProjectName::parse("app").unwrap();
    store
        .insert_project(&Project {
            name: name.clone(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: None,
            bots: Vec::new(),
        })
        .unwrap();
    store
        .insert_item(NewItem {
            project: name,
            branch: "work".into(),
            title: "Work".into(),
            state: ItemState::NeedsReview,
            backend: ItemBackend::Local,
            target: "main".into(),
            revision: NewRevision {
                head: "a".repeat(40),
                base: "0".repeat(40),
                files: Vec::new(),
                guide: None,
                interdiffs: Vec::new(),
                viewed: Vec::new(),
                parent: None,
            },
            priority: None,
            note: None,
        })
        .unwrap();
    (dir, store)
}

fn app() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn external(github_id: u64, login: &str, body: &str, anchored: bool) -> NewExternal {
    NewExternal {
        item: app(),
        revision: 1,
        anchor: anchored.then(|| Anchor {
            path: "src/login.rs".into(),
            side: Side::New,
            start_line: 2,
            line: 3,
            quote: "fn login() {\n}".into(),
        }),
        body: body.into(),
        kind: if anchored {
            PullCommentKind::ReviewComment
        } else {
            PullCommentKind::IssueComment
        },
        github_id,
        login: login.into(),
        bot: login.ends_with("[bot]"),
        url: format!("https://github.com/o/app/pull/1#c{github_id}"),
        path: anchored.then(|| "src/login.rs".into()),
        summary: false,
        dated: 0,
    }
}

fn reply(comment: CommentId, author: Author, status: Option<ReplyStatus>, body: &str) -> NewReply {
    NewReply {
        comment,
        item: app(),
        author,
        status,
        body: body.into(),
        revision: 1,
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[test]
fn external_comments_are_sent_from_the_start_and_wait_for_a_batch() {
    let (_dir, mut store) = store();
    let inline = store
        .insert_external(external(7, "codex[bot]", "Leaks", true), 100)
        .unwrap();
    let general = store
        .insert_external(external(8, "ana", "Why?", false), 200)
        .unwrap();
    assert_eq!(inline.anchor.as_ref().unwrap().quote, "fn login() {\n}");
    assert_eq!(general.anchor, None);
    let found = inline.external.clone().unwrap();
    assert_eq!(
        (found.kind, found.github_id, found.login.as_str(), found.bot),
        (PullCommentKind::ReviewComment, 7, "codex[bot]", true)
    );
    assert_eq!(found.path.as_deref(), Some("src/login.rs"));
    assert!(found.pending);
    assert!(!inline.is_draft());
    assert_eq!(store.comments(&app()).unwrap(), [inline.clone(), general]);
    assert_eq!(store.comment(inline.id).unwrap().unwrap(), inline);

    // Draft handling and a review leave them alone.
    store.set_draft_body(inline.id, "Changed").unwrap();
    store.delete_draft(inline.id).unwrap();
    let written = store
        .insert_review(
            NewReview {
                item: app(),
                revision: 1,
                verdict: Verdict::Comment,
                summary: "Looks fine".into(),
                moved: Vec::new(),
            },
            ItemState::NeedsReview,
        )
        .unwrap();
    assert!(written.comments.is_empty());
    assert_eq!(store.comment(inline.id).unwrap().unwrap(), inline);

    let held = store.held(&app()).unwrap().unwrap();
    assert_eq!((held.first, held.last), (100, 200));
}

#[test]
fn heard_counts_who_spoke_since_the_newest_push() {
    let (_dir, mut store) = store();
    assert_eq!(store.held(&app()).unwrap(), None);
    let before = now();
    store
        .insert_external(external(1, "old[bot]", "Stale", false), 1)
        .unwrap();
    let held = store.held(&app()).unwrap().unwrap();
    // The submit's stamp is close to now, to the millisecond.
    assert!(held.pushed.abs_diff(before) < 60_000, "{held:?}");
    assert!(held.heard.is_empty());

    // GitHub's date counts, not when the hub saw it: a comment posted
    // before the push and edited after it is heard, one GitHub dates before
    // it isn't, however late the hub saw it or its edit.
    let after = held.pushed + 1;
    let before = held.pushed - 1_000;
    let late = NewExternal {
        dated: before,
        ..external(2, "late[bot]", "Seen late", false)
    };
    let late = store.insert_external(late, after).unwrap();
    let stale = NewExternal {
        dated: before,
        ..external(3, "stale[bot]", "Seen late", false)
    };
    let stale = store.insert_external(stale, after).unwrap();
    store
        .edit_external(stale.id, "Edited before", before + 500, false, after)
        .unwrap();
    assert!(store.held(&app()).unwrap().unwrap().heard.is_empty());
    store
        .edit_external(late.id, "Edited late", held.pushed + 1_000, false, after)
        .unwrap();
    for (github_id, login, body) in [
        (4, "codex[bot]", "Leaks"),
        (5, "ana", "Why?"),
        (6, "codex[bot]", "Again"),
    ] {
        let new = NewExternal {
            dated: held.pushed,
            ..external(github_id, login, body, false)
        };
        store.insert_external(new, after).unwrap();
    }
    let held = store.held(&app()).unwrap().unwrap();
    assert_eq!(held.heard, ["late[bot]", "codex[bot]", "ana"]);
    assert_eq!((held.first, held.last), (1, after));
}

#[test]
fn a_release_logs_one_batch_that_keeps_its_bodies() {
    let (_dir, mut store) = store();
    let first = store
        .insert_external(external(7, "codex[bot]", "Leaks", true), 100)
        .unwrap();
    let second = store
        .insert_external(external(8, "ana", "Why?", false), 150)
        .unwrap();
    store.set_vote(first.id, Vote::Up, 150).unwrap();
    store.set_vote(second.id, Vote::Up, 150).unwrap();
    let released = store.release(&app(), 1).unwrap().unwrap();
    let EventKind::ExternalComments(batch) = &released.kind else {
        panic!("not a batch: {released:?}");
    };
    assert_eq!(batch.revision, 1);
    let carried: Vec<_> = batch
        .comments
        .iter()
        .map(|d| (d.comment.id, d.comment.body.as_str(), d.edited))
        .collect();
    assert_eq!(
        carried,
        [(first.id, "Leaks", false), (second.id, "Why?", false)]
    );
    assert_eq!(
        released.source(),
        mergegate_core::feedback::Source::External
    );
    assert_eq!(
        store.events(&app(), 0).unwrap(),
        std::slice::from_ref(&released)
    );
    assert_eq!(store.held(&app()).unwrap(), None);
    assert_eq!(store.release(&app(), 1).unwrap(), None);
    assert!(
        !store
            .comment(first.id)
            .unwrap()
            .unwrap()
            .external
            .unwrap()
            .pending
    );

    // An edit waits again, and goes out marked as one.
    store
        .edit_external(first.id, "Leaks a handle", 0, false, 300)
        .unwrap();
    store.set_vote(first.id, Vote::Up, 300).unwrap();
    let held = store.held(&app()).unwrap().unwrap();
    assert_eq!((held.first, held.last), (300, 300));
    let again = store.release(&app(), 1).unwrap().unwrap();
    let EventKind::ExternalComments(batch) = &again.kind else {
        panic!("not a batch: {again:?}");
    };
    let carried: Vec<_> = batch
        .comments
        .iter()
        .map(|d| (d.comment.id, d.comment.body.as_str(), d.edited))
        .collect();
    assert_eq!(carried, [(first.id, "Leaks a handle", true)]);
    // The first batch still reads as it went out.
    assert_eq!(store.events(&app(), 0).unwrap(), [released, again]);
}

#[test]
fn an_amend_changes_the_body_and_leaves_the_comment_where_it_stands() {
    let (_dir, mut store) = store();
    let sent = store
        .insert_external(external(1, "codex[bot]", "Leaks", false), 100)
        .unwrap();
    store.set_vote(sent.id, Vote::Up, 100).unwrap();
    store.release(&app(), 1).unwrap();
    store
        .amend_external(sent.id, "Leaks\n\nAddressed in abc123", 0, 200)
        .unwrap();
    let amended = store.comment(sent.id).unwrap().unwrap();
    assert_eq!(amended.body, "Leaks\n\nAddressed in abc123");
    let external = amended.external.unwrap();
    assert!(!external.pending);
    assert_eq!(external.vote, Some(Vote::Up));
    assert_eq!(store.held(&app()).unwrap(), None);
    assert_eq!(store.release(&app(), 1).unwrap(), None);
}

#[test]
fn a_withdrawn_comment_is_deleted_unless_it_reached_someone() {
    let (_dir, mut store) = store();
    let sent = store
        .insert_external(external(1, "ana", "Leaks", false), 100)
        .unwrap();
    store.set_vote(sent.id, Vote::Up, 100).unwrap();
    store.release(&app(), 1).unwrap();
    store
        .insert_reply(reply(sent.id, Author::Operator, None, "Where?"))
        .unwrap();
    store
        .edit_external(sent.id, "Leaks a handle", 0, false, 200)
        .unwrap();
    let drafted = store
        .insert_external(external(2, "ana", "Why?", false), 200)
        .unwrap();
    store
        .insert_reply(reply(drafted.id, Author::Operator, None, "Draft"))
        .unwrap();
    let answered = store
        .insert_external(external(3, "ana", "Rename it", false), 200)
        .unwrap();
    store
        .insert_reply(reply(
            answered.id,
            Author::Agent,
            Some(ReplyStatus::Addressed),
            "",
        ))
        .unwrap();

    for comment in [&sent, &drafted, &answered] {
        store.withdraw_external(comment.id).unwrap();
    }
    assert_eq!(store.held(&app()).unwrap(), None);
    let left: Vec<_> = store
        .comments(&app())
        .unwrap()
        .into_iter()
        .map(|c| (c.id, c.body, c.external.unwrap().pending))
        .collect();
    assert_eq!(
        left,
        [
            (sent.id, "Leaks".into(), false),
            (answered.id, "Rename it".into(), false)
        ]
    );
    // The operator's draft on the one kept stays; the deleted one's goes.
    let replies: Vec<_> = store
        .replies(&app())
        .unwrap()
        .into_iter()
        .map(|r| (r.comment, r.body))
        .collect();
    assert_eq!(
        replies,
        [(sent.id, "Where?".into()), (answered.id, String::new())]
    );
}

#[test]
fn an_override_goes_out_at_once_and_isnt_a_draft() {
    let (_dir, mut store) = store();
    let comment = store
        .insert_external(external(7, "codex[bot]", "Leaks", true), 100)
        .unwrap();
    store.set_vote(comment.id, Vote::Up, 100).unwrap();
    store.release(&app(), 1).unwrap();
    let rejection = store
        .insert_reply(reply(
            comment.id,
            Author::Agent,
            Some(ReplyStatus::Rejected),
            "It's closed on drop",
        ))
        .unwrap();
    store.set_agreed(comment.id, rejection.id).unwrap();
    let agreed = store.comment(comment.id).unwrap().unwrap();
    assert_eq!(agreed.external.unwrap().agreed_reply, Some(rejection.id));

    let (written, event) = store
        .insert_override(reply(comment.id, Author::Operator, None, ""))
        .unwrap();
    assert!(written.overrides);
    assert!(!written.is_draft());
    let EventKind::Override(found) = &event.kind else {
        panic!("not an override: {event:?}");
    };
    assert_eq!(found.reply, written);
    assert_eq!(found.comment.id, comment.id);
    assert_eq!(store.events(&app(), 0).unwrap().last(), Some(&event));

    // Neither draft handling nor the next review touches it.
    store.set_draft_reply_body(written.id, "Changed").unwrap();
    store.delete_draft_reply(written.id).unwrap();
    let review = store
        .insert_review(
            NewReview {
                item: app(),
                revision: 1,
                verdict: Verdict::Comment,
                summary: "".into(),
                moved: Vec::new(),
            },
            ItemState::NeedsReview,
        )
        .unwrap();
    assert!(review.replies.is_empty());
    assert_eq!(store.reply(written.id).unwrap().unwrap(), written);
}

#[test]
fn the_table_keeps_comments_well_formed() {
    let (_dir, mut store) = store();
    let review = store
        .insert_review(
            NewReview {
                item: app(),
                revision: 1,
                verdict: Verdict::Comment,
                summary: "".into(),
                moved: Vec::new(),
            },
            ItemState::NeedsReview,
        )
        .unwrap();
    let insert = |source: &str, path: Option<&str>, review: Option<u64>| {
        store.conn.execute(
            "INSERT INTO comments (item_id, revision, source, review_id, path, side,
                                   start_line, line, body, quote, created_at)
             VALUES (1, 1, ?1, ?2, ?3, CASE WHEN ?3 IS NULL THEN NULL ELSE 'new' END,
                     CASE WHEN ?3 IS NULL THEN NULL ELSE 1 END,
                     CASE WHEN ?3 IS NULL THEN NULL ELSE 1 END, 'x',
                     CASE WHEN ?3 IS NULL THEN NULL ELSE 'q' END, 'now')",
            params![source, review.map(|r| r as i64), path],
        )
    };
    // Only an external comment may be on the item as a whole, and no review
    // sends one.
    assert!(insert("operator", None, None).is_err());
    assert!(insert("external", None, None).is_ok());
    assert!(insert("external", Some("a.rs"), Some(review.id)).is_err());
    assert!(insert("operator", Some("a.rs"), Some(review.id)).is_ok());
    // Lines come whole or not at all.
    let half = store.conn.execute(
        "INSERT INTO comments (item_id, revision, source, path, body, created_at)
         VALUES (1, 1, 'external', 'a.rs', 'x', 'now')",
        [],
    );
    assert!(half.is_err());
}

#[test]
fn a_comment_goes_out_only_once_voted_up() {
    let (_dir, mut store) = store();
    let carried = |event: Option<Event>| -> Vec<CommentId> {
        let Some(Event {
            kind: EventKind::ExternalComments(batch),
            ..
        }) = event
        else {
            return Vec::new();
        };
        batch.comments.iter().map(|d| d.comment.id).collect()
    };
    let external_of = |store: &Store, id| store.comment(id).unwrap().unwrap().external.unwrap();
    let mut insert = |github_id, login, body| {
        store
            .insert_external(external(github_id, login, body, false), 100)
            .unwrap()
            .id
    };
    let person = insert(1, "ana", "Why?");
    let useful = insert(2, "codex[bot]", "Leaks");
    let noise = insert(3, "codex[bot]", "Rename");
    assert!(store.held(&app()).unwrap().unwrap().awaiting);
    assert_eq!(store.release(&app(), 1).unwrap(), None);

    store.set_vote(person, Vote::Up, 200).unwrap();
    store.set_vote(useful, Vote::Up, 200).unwrap();
    assert_eq!(carried(store.release(&app(), 1).unwrap()), [person, useful]);
    assert!(store.held(&app()).unwrap().unwrap().awaiting);
    store.set_vote(noise, Vote::Down, 300).unwrap();
    assert_eq!(store.held(&app()).unwrap(), None);
    let dismissed = external_of(&store, noise);
    assert_eq!(
        (dismissed.vote, dismissed.pending),
        (Some(Vote::Down), false)
    );

    // An edit asks for a new vote, unless the operator voted it down.
    store
        .edit_external(useful, "Leaks more", 0, false, 400)
        .unwrap();
    store
        .edit_external(noise, "Rename it", 0, false, 400)
        .unwrap();
    assert!(external_of(&store, useful).awaiting_vote());
    let dismissed = external_of(&store, noise);
    assert!(dismissed.dismissed() && !dismissed.pending);
    assert!(store.held(&app()).unwrap().unwrap().awaiting);

    let forced = store.conn.execute(
        "UPDATE external_comments SET vote = 'maybe' WHERE comment_id = ?1",
        params![person.0 as i64],
    );
    assert!(forced.is_err());
}

#[test]
fn a_summary_is_never_held() {
    let (_dir, mut store) = store();
    let external_of = |store: &Store, id| store.comment(id).unwrap().unwrap().external.unwrap();
    let new = NewExternal {
        summary: true,
        ..external(1, "coderabbitai[bot]", "Walkthrough", false)
    };
    let summary = store.insert_external(new, 100).unwrap().id;
    let found = external_of(&store, summary);
    assert!(found.summary && !found.pending);
    assert_eq!(store.held(&app()).unwrap(), None);

    // One held, and voted up, before it was told apart.
    let held = store
        .insert_external(external(2, "codex[bot]", "Review summary", false), 100)
        .unwrap()
        .id;
    store.set_vote(held, Vote::Up, 200).unwrap();
    store
        .edit_external(held, "Review summary", 0, true, 300)
        .unwrap();
    let found = external_of(&store, held);
    assert!(found.summary && !found.pending && found.vote.is_none());
    assert_eq!(store.held(&app()).unwrap(), None);
    assert_eq!(store.release(&app(), 1).unwrap(), None);
}
