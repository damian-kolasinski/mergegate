use super::*;
use crate::fake::{FakeGit, FakeHighlighter, FakeStore, commit, installed};
use crate::highlight::{Span, Tag};
use crate::item::{ItemId, ItemState};
use crate::project::ProjectName;

fn blobs(pending: &[Pending]) -> Vec<(&str, &str, &str)> {
    pending
        .iter()
        .map(|p| (p.blob.as_str(), p.grammar.as_str(), p.version.as_str()))
        .collect()
}

#[test]
fn lists_both_sides_of_each_file_an_installed_grammar_covers() {
    // r1 adds src/login.rs and changes README.md; r2 adds src/r2.rs.
    let mut store = FakeStore::with_item(2);
    let pending = pending(&mut store, &installed(&[("rs", "1")])).unwrap();
    let r2 = format!("{}\n", commit(3));
    let login = format!("{}\n", commit(2));
    assert_eq!(
        blobs(&pending),
        [(login.as_str(), "rs", "rs@1"), (r2.as_str(), "rs", "rs@1")]
    );
    assert!(pending.iter().all(|p| p.project.to_string() == "app"));
}

#[test]
fn skips_blobs_already_highlighted_and_closed_items() {
    let mut store = FakeStore::with_item(1);
    let packs = installed(&[("rs", "1"), ("md", "1")]);
    let first = pending(&mut store, &packs).unwrap();
    assert_eq!(first.len(), 3, "README's two sides and login.rs: {first:?}");
    let done = &first[0];
    let spans = Highlights::default();
    store
        .insert_highlights(&done.blob, &done.grammar, &done.version, &spans)
        .unwrap();
    assert_eq!(pending(&mut store, &packs).unwrap(), first[1..]);

    store.set_state(&ItemId::parse("app#1").unwrap(), ItemState::Abandoned);
    assert_eq!(pending(&mut store, &packs).unwrap(), []);
}

#[test]
fn drops_spans_computed_with_other_packs_installed() {
    let mut store = FakeStore::with_item(1);
    let spans = Highlights::default();
    for (grammar, version) in [("rs", "rs@1"), ("rs", "rs@2"), ("md", "rs@2,md@1")] {
        store
            .insert_highlights("b", grammar, version, &spans)
            .unwrap();
    }
    pending(&mut store, &installed(&[("rs", "2")])).unwrap();
    assert_eq!(
        store.highlighted(),
        [("b".into(), "rs".into(), "rs@2".into())]
    );
}

#[test]
fn highlights_again_once_another_pack_is_installed() {
    // A Markdown file's fences may now inject the new grammar.
    let mut store = FakeStore::with_item(1);
    let before = installed(&[("md", "1")]);
    for done in pending(&mut store, &before).unwrap() {
        store
            .insert_highlights(
                &done.blob,
                &done.grammar,
                &done.version,
                &Highlights::default(),
            )
            .unwrap();
    }
    assert_eq!(pending(&mut store, &before).unwrap(), []);
    let after = installed(&[("md", "1"), ("rs", "1")]);
    let again = pending(&mut store, &after).unwrap();
    assert_eq!(again.len(), 3, "README's two sides and login.rs: {again:?}");
    assert_eq!(store.highlighted(), []);
}

#[test]
fn computes_spans_from_the_blob_in_staging() {
    let highlighter = FakeHighlighter::with(&[("rs", "1")]);
    let pending = Pending {
        project: ProjectName::parse("app").unwrap(),
        blob: "fn é\n".into(),
        grammar: "rs".into(),
        version: "1".into(),
    };
    let spans = compute(&FakeGit::default(), &highlighter, &pending).unwrap();
    let keyword = Span {
        start: 0,
        len: 4,
        tag: Tag::Keyword,
    };
    assert_eq!(spans.lines, [vec![keyword], vec![]]);
}

#[test]
fn computes_no_spans_for_a_grammar_that_is_gone_or_a_huge_blob() {
    let highlighter = FakeHighlighter::with(&[("rs", "1")]);
    let mut pending = Pending {
        project: ProjectName::parse("app").unwrap(),
        blob: "x".repeat(MAX_BYTES + 1),
        grammar: "rs".into(),
        version: "1".into(),
    };
    let git = FakeGit::default();
    assert_eq!(
        compute(&git, &highlighter, &pending).unwrap(),
        Highlights::default()
    );
    pending.blob = "x".into();
    pending.grammar = "md".into();
    assert_eq!(
        compute(&git, &highlighter, &pending).unwrap(),
        Highlights::default()
    );
}
