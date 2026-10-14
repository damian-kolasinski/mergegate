use super::*;
use crate::fake::{FakeGit, FakeStore};
use crate::review::{
    ReplyStatus, Side, SubmitReview, Verdict, WriteDraft, WriteReply, reply, submit, write_draft,
    write_draft_reply,
};

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn draft(revision: u32, body: &str) -> WriteDraft {
    WriteDraft {
        id: id("app#1"),
        revision,
        path: "README.md".into(),
        side: Side::New,
        start_line: 1,
        line: 1,
        body: body.into(),
    }
}

#[test]
fn holds_every_revisions_comments_and_replies() {
    let mut store = FakeStore::with_item(2);
    let git = FakeGit::default();
    write_draft(&mut store, &git, draft(1, "Why?")).unwrap();
    let review = SubmitReview {
        id: id("app#1"),
        revision: 1,
        verdict: Verdict::ChangesRequested,
        summary: String::new(),
    };
    submit(&mut store, review).unwrap();
    let question = WriteReply {
        comment: crate::review::CommentId(1),
        status: ReplyStatus::Question,
        body: "Which case?".into(),
    };
    reply(&mut store, question).unwrap();
    write_draft_reply(&mut store, crate::review::CommentId(1), "Both.".into()).unwrap();
    write_draft(&mut store, &git, draft(2, "And here?")).unwrap();

    let found = conversation(&store, &id("app#1")).unwrap().unwrap();
    assert_eq!(found.newest, 2);
    let comments: Vec<_> = found
        .comments
        .iter()
        .map(|c| (c.revision, c.body.as_str(), c.review))
        .collect();
    assert_eq!(comments, [(1, "Why?", Some(1)), (2, "And here?", None)]);
    let replies: Vec<_> = found
        .replies
        .iter()
        .map(|r| (r.body.as_str(), r.is_draft()))
        .collect();
    assert_eq!(replies, [("Which case?", false), ("Both.", true)]);
}

#[test]
fn an_unknown_item_has_none() {
    let store = FakeStore::with_item(1);
    assert_eq!(conversation(&store, &id("app#2")).unwrap(), None);
}
