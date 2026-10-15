use mergegate_core::feedback::{EventKind, FeedbackStore};
use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};
use mergegate_core::review::{Comment, NewComment, NewReview, ReviewStore, Side, Verdict};

use super::*;
use crate::Store;

/// `app#1` and `app#2`, each with one comment a review sent.
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
    for branch in ["work", "other"] {
        let item = store
            .insert_item(NewItem {
                project: name.clone(),
                branch: branch.into(),
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
                },
            })
            .unwrap();
        store
            .insert_draft(NewComment {
                item: item.id.clone(),
                revision: 1,
                path: "src/login.rs".into(),
                side: Side::New,
                start_line: 1,
                line: 1,
                body: "Why?".into(),
                quote: "fn login() {}".into(),
            })
            .unwrap();
        let review = NewReview {
            item: item.id,
            revision: 1,
            verdict: Verdict::ChangesRequested,
            summary: String::new(),
            moved: Vec::new(),
        };
        store
            .insert_review(review, ItemState::ChangesRequested)
            .unwrap();
    }
    (dir, store)
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn agent(comment: u64, status: ReplyStatus, body: &str) -> NewReply {
    let item = if comment == 1 { "app#1" } else { "app#2" };
    NewReply {
        comment: CommentId(comment),
        item: id(item),
        author: Author::Agent,
        status: Some(status),
        body: body.into(),
        revision: 1,
    }
}

fn operator(comment: u64, body: &str) -> NewReply {
    NewReply {
        author: Author::Operator,
        status: None,
        body: body.into(),
        ..agent(comment, ReplyStatus::Addressed, "")
    }
}

#[test]
fn replies_thread_on_their_items_comments() {
    let (_dir, mut store) = store();
    let addressed = store
        .insert_reply(agent(1, ReplyStatus::Addressed, ""))
        .unwrap();
    assert_eq!(addressed.comment, CommentId(1));
    assert_eq!(addressed.item, id("app#1"));
    assert_eq!(addressed.author, Author::Agent);
    assert_eq!(addressed.status, Some(ReplyStatus::Addressed));
    assert_eq!((addressed.revision, addressed.review), (1, None));
    assert!(addressed.created_at.ends_with('Z'));
    let elsewhere = store
        .insert_reply(agent(2, ReplyStatus::Question, "Which one?"))
        .unwrap();
    let rejected = store
        .insert_reply(agent(1, ReplyStatus::Rejected, "No."))
        .unwrap();

    assert_eq!(
        store.replies(&id("app#1")).unwrap(),
        [addressed, rejected.clone()]
    );
    assert_eq!(store.replies(&id("app#2")).unwrap(), [elsewhere]);
    assert_eq!(rejected.id, 3);
}

#[test]
fn the_table_keeps_replies_well_formed() {
    let (_dir, mut store) = store();
    let mut silent = agent(1, ReplyStatus::Addressed, "");
    silent.status = None;
    store.insert_reply(silent).unwrap_err();
    let operator = NewReply {
        author: Author::Operator,
        ..agent(1, ReplyStatus::Addressed, "Thanks")
    };
    store.insert_reply(operator).unwrap_err();
    store
        .insert_reply(agent(1, ReplyStatus::Rejected, ""))
        .unwrap_err();
    store
        .insert_reply(agent(9, ReplyStatus::Addressed, ""))
        .unwrap_err();
    assert_eq!(store.replies(&id("app#1")).unwrap(), []);
}

#[test]
fn a_comment_is_found_by_its_id_alone() {
    let (_dir, store) = store();
    let found: Comment = store.comment(CommentId(2)).unwrap().unwrap();
    assert_eq!(found.item, id("app#2"));
    assert_eq!(found.body, "Why?");
    assert_eq!(found.review, Some(2));
    assert_eq!(store.comment(CommentId(3)).unwrap(), None);
}

#[test]
fn the_next_review_sends_its_items_draft_replies() {
    let (_dir, mut store) = store();
    let question = store
        .insert_reply(agent(1, ReplyStatus::Question, "Empty or missing?"))
        .unwrap();
    let draft = store.insert_reply(operator(1, "Both.")).unwrap();
    let elsewhere = store.insert_reply(operator(2, "Later.")).unwrap();
    assert_eq!(
        (draft.author, draft.status, draft.review),
        (Author::Operator, None, None)
    );
    store
        .set_draft_reply_body(draft.id, "Both, please.")
        .unwrap();
    assert_eq!(
        store.reply(draft.id).unwrap().unwrap().body,
        "Both, please."
    );

    let review = NewReview {
        item: id("app#1"),
        revision: 1,
        verdict: Verdict::Comment,
        summary: String::new(),
        moved: Vec::new(),
    };
    let sent = store.insert_review(review, ItemState::NeedsReview).unwrap();
    let draft = Reply {
        review: Some(sent.id),
        body: "Both, please.".into(),
        ..draft
    };
    assert_eq!(sent.replies, std::slice::from_ref(&draft));
    let events = store.events(&id("app#1"), 0).unwrap();
    assert_eq!(events[1].kind, EventKind::ReviewSubmitted(sent));
    assert_eq!(
        store.replies(&id("app#1")).unwrap(),
        [question, draft.clone()]
    );
    assert_eq!(store.reply(elsewhere.id).unwrap(), Some(elsewhere.clone()));

    // Sent, it stays as the agent read it.
    store.set_draft_reply_body(draft.id, "Neither.").unwrap();
    store.delete_draft_reply(draft.id).unwrap();
    assert_eq!(store.reply(draft.id).unwrap(), Some(draft));
    store.delete_draft_reply(elsewhere.id).unwrap();
    assert_eq!(store.reply(elsewhere.id).unwrap(), None);
}
