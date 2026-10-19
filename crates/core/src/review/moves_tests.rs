use super::*;
use crate::fake::{FakeGit, FakeStore, remapped};
use crate::review::{
    Lines, MoveDraft, Side, SubmitReview, Verdict, WriteDraft, delete_draft, move_draft, submit,
    write_draft,
};

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn draft(store: &mut FakeStore, git: &FakeGit, path: &str, side: Side) -> Comment {
    let request = WriteDraft {
        id: id(),
        revision: 1,
        path: path.into(),
        side,
        start_line: 2,
        line: 2,
        body: "Why?".into(),
    };
    write_draft(store, git, request).unwrap()
}

fn review(revision: u32) -> SubmitReview {
    SubmitReview {
        id: id(),
        revision,
        verdict: Verdict::Comment,
        summary: String::new(),
    }
}

#[test]
fn a_review_moves_drafts_whose_lines_didnt_change() {
    let (mut store, git) = remapped(true);
    let new = draft(&mut store, &git, "src/moved.rs", Side::New);
    let old = draft(&mut store, &git, "src/moved.rs", Side::Old);

    let sent = submit(&mut store, &git, review(2)).unwrap().review;
    let placed: Vec<_> = sent
        .comments
        .iter()
        .map(|c| {
            let anchor = c.anchor.as_ref().unwrap();
            (
                c.id,
                c.revision,
                anchor.start_line,
                anchor.line,
                anchor.quote.as_str(),
            )
        })
        .collect();
    assert_eq!(placed, [(new.id, 2, 3, 3, "B"), (old.id, 2, 2, 2, "b")]);
    assert_eq!(store.comments(&id()).unwrap(), sent.comments);
}

#[test]
fn a_review_waits_for_drafts_whose_lines_changed() {
    let (mut store, git) = remapped(false);
    let moved = draft(&mut store, &git, "src/moved.rs", Side::New);
    let changed = draft(&mut store, &git, "src/changed.rs", Side::New);
    let gone = draft(&mut store, &git, "src/gone.rs", Side::New);

    let err = submit(&mut store, &git, review(2)).unwrap_err();
    let ReviewError::Unplaced { drafts, .. } = err else {
        panic!("{err}");
    };
    assert_eq!(drafts, [changed.id, gone.id]);
    assert!(store.reviews().is_empty());
    // A review of the revision they were written on sends them as they are.
    assert!(move_drafts(&store, &git, &id(), 1).unwrap().is_empty());

    let lines = Lines {
        revision: 2,
        path: "src/changed.rs".into(),
        side: Side::New,
        start_line: 2,
        line: 2,
    };
    let request = MoveDraft {
        id: id(),
        comment: changed.id,
        lines,
    };
    let replaced = move_draft(&mut store, &git, request).unwrap();
    assert_eq!(replaced.revision, 2);
    assert_eq!(replaced.anchor.as_ref().unwrap().quote, "B2");
    delete_draft(&mut store, &id(), gone.id).unwrap();

    let sent = submit(&mut store, &git, review(2)).unwrap().review;
    let placed: Vec<_> = sent.comments.iter().map(|c| (c.id, c.revision)).collect();
    assert_eq!(placed, [(moved.id, 2), (changed.id, 2)]);
}
