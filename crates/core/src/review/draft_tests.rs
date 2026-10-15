use super::*;
use crate::fake::{FakeGit, FakeStore, commit};
use crate::item::ItemState;
use crate::review::{SubmitReview, Verdict, submit};

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

/// On `app#1`'s r1, where `README.md` goes from c1's line to c1's and c2's.
fn on(path: &str, side: Side, start_line: u32, line: u32) -> WriteDraft {
    WriteDraft {
        id: id("app#1"),
        revision: 1,
        path: path.into(),
        side,
        start_line,
        line,
        body: "  Why this?\n".into(),
    }
}

#[test]
fn a_draft_quotes_the_lines_it_is_on() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let range = write_draft(&mut store, &git, on("README.md", Side::New, 1, 2)).unwrap();
    assert_eq!(range.id, CommentId(1));
    assert_eq!(range.body, "Why this?");
    assert_eq!(
        range.anchor.as_ref().unwrap().quote,
        format!("{}\n{}", commit(1), commit(2))
    );
    assert_eq!(range.review, None);

    let old = write_draft(&mut store, &git, on("README.md", Side::Old, 1, 1)).unwrap();
    assert_eq!(old.id, CommentId(2));
    assert_eq!(old.anchor.as_ref().unwrap().quote, commit(1));
    assert_eq!(store.comments(&id("app#1")).unwrap(), [range, old]);
}

#[test]
fn a_quote_keeps_the_last_lines_of_a_long_range() {
    let text = "1\n2\n3\n4\n5\n6\n7\n";
    assert_eq!(quote(text, 1, 7).unwrap(), "3\n4\n5\n6\n7");
    assert_eq!(quote(text, 4, 6).unwrap(), "4\n5\n6");
    assert_eq!(quote(text, 7, 7).unwrap(), "7");
    assert_eq!(quote(text, 7, 8), None);
}

#[test]
fn a_quote_counts_lines_as_git_does() {
    assert_eq!(quote("a\r\nb\r\n", 1, 2).unwrap(), "a\nb");
    assert_eq!(quote("a\nb", 2, 2).unwrap(), "b");
    assert_eq!(quote("a\n", 2, 2), None);
}

#[test]
fn a_quote_cuts_long_lines() {
    let long = "x".repeat(QUOTE_LINE_CHARS + 50);
    let quoted = quote(&format!("é{long}\nshort\n"), 1, 2).unwrap();
    let (first, rest) = quoted.split_once('\n').unwrap();
    assert_eq!(first.chars().count(), QUOTE_LINE_CHARS + 1);
    assert!(first.starts_with('é') && first.ends_with('…'));
    assert_eq!(rest, "short");
}

#[test]
fn refuses_drafts_on_lines_the_revision_doesnt_have() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let mut err = |request| {
        write_draft(&mut store, &git, request)
            .unwrap_err()
            .to_string()
    };
    assert_eq!(
        err(WriteDraft {
            id: id("app#2"),
            ..on("README.md", Side::New, 1, 1)
        }),
        "no item app#2"
    );
    assert_eq!(
        err(WriteDraft {
            revision: 2,
            ..on("README.md", Side::New, 1, 1)
        }),
        "app#1 has no revision 2"
    );
    assert_eq!(
        err(WriteDraft {
            body: " \n".into(),
            ..on("README.md", Side::New, 1, 1)
        }),
        "a comment needs a body"
    );
    assert_eq!(
        err(on("src/other.rs", Side::New, 1, 1)),
        "app#1 r1 doesn't change src/other.rs"
    );
    // An added file has no old side.
    assert_eq!(
        err(on("src/login.rs", Side::Old, 1, 1)),
        "the diff of src/login.rs shows no lines 1-1 on the old side"
    );
    for (start_line, line) in [(0, 1), (2, 1), (2, 3)] {
        assert_eq!(
            err(on("README.md", Side::New, start_line, line)),
            format!("the diff of README.md shows no lines {start_line}-{line} on the new side")
        );
    }
    store.set_state(&id("app#1"), ItemState::Merged);
    let closed = write_draft(&mut store, &git, on("README.md", Side::New, 1, 1)).unwrap_err();
    assert_eq!(closed.to_string(), "app#1 is closed");
    assert_eq!(store.comments(&id("app#1")).unwrap(), []);
}

#[test]
fn a_closed_item_keeps_its_drafts_only_to_delete() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let draft = write_draft(&mut store, &git, on("README.md", Side::New, 1, 1)).unwrap();
    store.set_state(&id("app#1"), ItemState::Abandoned);
    let edited = edit_draft(&mut store, &id("app#1"), draft.id, "Later.".into()).unwrap_err();
    assert_eq!(edited.to_string(), "app#1 is closed");
    delete_draft(&mut store, &id("app#1"), draft.id).unwrap();
    assert_eq!(store.comments(&id("app#1")).unwrap(), []);
}

#[test]
fn drafts_change_until_a_review_sends_them() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let first = write_draft(&mut store, &git, on("README.md", Side::New, 1, 1)).unwrap();
    let second = write_draft(&mut store, &git, on("README.md", Side::New, 2, 2)).unwrap();

    let edited = edit_draft(&mut store, &id("app#1"), first.id, " Better.\n".into()).unwrap();
    assert_eq!(edited.body, "Better.");
    assert_eq!(store.comments(&id("app#1")).unwrap()[0].body, "Better.");
    let empty = edit_draft(&mut store, &id("app#1"), first.id, " ".into()).unwrap_err();
    assert_eq!(empty.to_string(), "a comment needs a body");

    delete_draft(&mut store, &id("app#1"), second.id).unwrap();
    let gone = delete_draft(&mut store, &id("app#1"), second.id).unwrap_err();
    assert_eq!(gone.to_string(), "app#1 has no draft c_2");
    let missing = edit_draft(&mut store, &id("app#2"), first.id, "x".into()).unwrap_err();
    assert_eq!(missing.to_string(), "no item app#2");

    let request = SubmitReview {
        id: id("app#1"),
        revision: 1,
        verdict: Verdict::Comment,
        summary: String::new(),
    };
    submit(&mut store, &FakeGit::default(), request).unwrap();
    let sent = edit_draft(&mut store, &id("app#1"), first.id, "Later.".into()).unwrap_err();
    assert_eq!(sent.to_string(), "c_1 was sent, so it can't change");
    let sent = delete_draft(&mut store, &id("app#1"), first.id).unwrap_err();
    assert_eq!(sent.to_string(), "c_1 was sent, so it can't change");
    let sent = move_draft(&mut store, &git, moving(first.id, 2, 2)).unwrap_err();
    assert_eq!(sent.to_string(), "c_1 was sent, so it can't change");
}

fn moving(comment: CommentId, start_line: u32, line: u32) -> MoveDraft {
    MoveDraft {
        id: id("app#1"),
        comment,
        lines: Lines {
            revision: 1,
            path: "README.md".into(),
            side: Side::New,
            start_line,
            line,
        },
    }
}

#[test]
fn a_draft_moves_only_onto_lines_a_new_one_could_go_on() {
    let mut store = FakeStore::with_item(1);
    let git = FakeGit::default();
    let draft = write_draft(&mut store, &git, on("README.md", Side::New, 1, 1)).unwrap();
    let refused = move_draft(&mut store, &git, moving(draft.id, 2, 3)).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "the diff of README.md shows no lines 2-3 on the new side"
    );
    assert_eq!(
        store.comments(&id("app#1")).unwrap(),
        std::slice::from_ref(&draft)
    );

    let moved = move_draft(&mut store, &git, moving(draft.id, 2, 2)).unwrap();
    assert_eq!((moved.id, moved.body.as_str()), (draft.id, "Why this?"));
    assert_eq!(moved.anchor.as_ref().unwrap().line, 2);
    assert_eq!(store.comments(&id("app#1")).unwrap(), [moved]);
}
