use mergegate_core::feedback::{EventKind, FeedbackStore};
use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};
use mergegate_core::review::{CommentId, NewComment, NewReview, Side, Verdict};

use super::*;

/// `app#1` with two revisions.
fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    let revision = || NewRevision {
        head: "a".repeat(40),
        base: "0".repeat(40),
        files: Vec::new(),
    };
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
            revision: revision(),
        })
        .unwrap();
    let app = ItemId::parse("app#1").unwrap();
    store
        .add_revision(&app, "Work", ItemState::NeedsReview, revision())
        .unwrap();
    (dir, store)
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn review(item: &str, revision: u32, summary: &str) -> NewReview {
    NewReview {
        item: id(item),
        revision,
        verdict: Verdict::ChangesRequested,
        summary: summary.into(),
    }
}

#[test]
fn a_review_moves_the_item_and_logs_one_event() {
    let (_dir, mut store) = store();
    let app = id("app#1");
    let written = store
        .insert_review(review("app#1", 1, "Fix it"), ItemState::ChangesRequested)
        .unwrap();
    assert_eq!(written.revision, 1);
    assert_eq!(written.summary, "Fix it");
    assert!(written.submitted_at.ends_with('Z'));
    let item = store.item(&app).unwrap().unwrap();
    assert_eq!(item.state, ItemState::ChangesRequested);
    // The review doesn't add a revision.
    assert_eq!(item.revision.number, 2);

    let events = store.events(&app, 0).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::ReviewSubmitted(written));
    assert_eq!(store.last_cursor(&app).unwrap(), events[0].cursor);
}

fn draft(revision: u32, body: &str) -> NewComment {
    NewComment {
        item: id("app#1"),
        revision,
        path: "src/login.rs".into(),
        side: Side::Old,
        start_line: 2,
        line: 3,
        body: body.into(),
        quote: "fn login() {\n}".into(),
    }
}

#[test]
fn drafts_change_until_a_review_sends_them() {
    let (_dir, mut store) = store();
    let app = id("app#1");
    let first = store.insert_draft(draft(1, "First")).unwrap();
    assert_eq!(first.id, CommentId(1));
    let anchor = first.anchor.as_ref().unwrap();
    assert_eq!(
        (anchor.side, anchor.start_line, anchor.line),
        (Side::Old, 2, 3)
    );
    assert_eq!(anchor.quote, "fn login() {\n}");
    assert_eq!(first.external, None);
    assert_eq!(first.review, None);
    let second = store.insert_draft(draft(2, "Second")).unwrap();
    store.set_draft_body(first.id, "First, again").unwrap();
    store.delete_draft(second.id).unwrap();
    // A deleted draft's ID isn't given out again.
    let third = store.insert_draft(draft(2, "Third")).unwrap();
    assert_eq!(third.id, CommentId(3));

    let written = store
        .insert_review(review("app#1", 2, ""), ItemState::ChangesRequested)
        .unwrap();
    let sent: Vec<_> = written
        .comments
        .iter()
        .map(|c| (c.id, c.revision, c.body.as_str(), c.review))
        .collect();
    assert_eq!(
        sent,
        [
            (first.id, 1, "First, again", Some(written.id)),
            (third.id, 2, "Third", Some(written.id)),
        ]
    );
    assert_eq!(store.comments(&app).unwrap(), written.comments);
    let events = store.events(&app, 0).unwrap();
    assert_eq!(events[0].kind, EventKind::ReviewSubmitted(written.clone()));

    // Sent comments stay as the agent read them.
    store.set_draft_body(first.id, "Changed").unwrap();
    store.delete_draft(third.id).unwrap();
    assert_eq!(store.comments(&app).unwrap(), written.comments);
    let next = store
        .insert_review(review("app#1", 2, "More"), ItemState::ChangesRequested)
        .unwrap();
    assert_eq!(next.comments, []);
}

#[test]
fn a_review_sends_only_its_items_drafts() {
    let (_dir, mut store) = store();
    store
        .insert_item(NewItem {
            project: ProjectName::parse("app").unwrap(),
            branch: "other".into(),
            title: "Other".into(),
            state: ItemState::NeedsReview,
            backend: ItemBackend::Local,
            target: "main".into(),
            revision: NewRevision {
                head: "b".repeat(40),
                base: "0".repeat(40),
                files: Vec::new(),
            },
        })
        .unwrap();
    let other = id("app#2");
    let mine = store.insert_draft(draft(1, "Mine")).unwrap();
    let elsewhere = store
        .insert_draft(NewComment {
            item: other.clone(),
            ..draft(1, "Elsewhere")
        })
        .unwrap();

    let written = store
        .insert_review(review("app#1", 2, ""), ItemState::ChangesRequested)
        .unwrap();
    assert_eq!(
        written.comments.iter().map(|c| c.id).collect::<Vec<_>>(),
        [mine.id]
    );
    assert_eq!(store.comments(&id("app#1")).unwrap(), written.comments);
    assert_eq!(store.comments(&other).unwrap(), [elsewhere]);
}
