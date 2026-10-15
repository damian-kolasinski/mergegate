use mergegate_core::interdiff::{Change, InterdiffFile, NewInterdiff};
use mergegate_core::item::{
    ChangedFile, FileStat, ItemBackend, ItemState, ItemStore, NewItem, NewRevision,
};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};

use super::*;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("hub.db")).unwrap();
    (dir, store)
}

fn span(start: u32, len: u32, tag: Tag) -> Span {
    Span { start, len, tag }
}

fn spans() -> Highlights {
    Highlights {
        lines: vec![
            vec![span(0, 2, Tag::Keyword), span(3, 4, Tag::Function)],
            vec![],
            vec![span(4, 1, Tag::Punctuation)],
            vec![],
        ],
    }
}

#[test]
fn round_trips_spans_by_blob_grammar_and_version() {
    let (_dir, mut store) = store();
    let none = Highlights::default();
    store.insert_highlights("a", "rust", "1", &spans()).unwrap();
    store.insert_highlights("b", "rust", "1", &none).unwrap();
    assert_eq!(store.highlights("a", "rust", "1").unwrap(), Some(spans()));
    assert_eq!(store.highlights("b", "rust", "1").unwrap(), Some(none));
    assert!(store.has_highlights("b", "rust", "1").unwrap());
    for (blob, grammar, version) in [("a", "rust", "2"), ("a", "toml", "1"), ("c", "rust", "1")] {
        assert_eq!(store.highlights(blob, grammar, version).unwrap(), None);
        assert!(!store.has_highlights(blob, grammar, version).unwrap());
    }
}

#[test]
fn reads_only_the_wanted_lines() {
    let (_dir, mut store) = store();
    store.insert_highlights("a", "rust", "1", &spans()).unwrap();
    let some = store.highlight_lines("a", "rust", "1", &[1, 3]).unwrap();
    assert_eq!(
        some,
        Some(spans()).map(|mut h| {
            h.lines.truncate(3);
            h
        })
    );
    let third = store
        .highlight_lines("a", "rust", "1", &[3])
        .unwrap()
        .unwrap();
    assert_eq!(third.lines, [vec![], vec![], spans().lines[2].clone()]);
    assert_eq!(store.highlight_lines("a", "rust", "2", &[1]).unwrap(), None);
}

#[test]
fn every_tag_round_trips() {
    let (_dir, mut store) = store();
    let all = Highlights {
        lines: vec![Tag::ALL.iter().map(|&tag| span(1, 2, tag)).collect()],
    };
    store.insert_highlights("a", "rust", "1", &all).unwrap();
    assert_eq!(store.highlights("a", "rust", "1").unwrap(), Some(all));
}

#[test]
fn stores_a_line_of_text_per_line() {
    let (_dir, mut store) = store();
    store.insert_highlights("a", "rust", "1", &spans()).unwrap();
    let lines: String = store
        .conn
        .query_row("SELECT lines FROM highlights", [], |row| row.get(0))
        .unwrap();
    assert_eq!(lines, "0,2,keyword 3,4,function\n\n4,1,punctuation\n\n");
}

#[test]
fn rejects_a_malformed_span() {
    for lines in ["1,2\n", "1,2,keyword,3\n", "x,2,keyword\n", "1,2,bold\n"] {
        assert!(decode(lines).is_err(), "{lines:?}");
    }
}

#[test]
fn keeps_only_the_spans_of_one_version() {
    let (_dir, mut store) = store();
    let none = Highlights::default();
    for (blob, grammar, version) in [("a", "rust", "1"), ("a", "rust", "2"), ("b", "toml", "2")] {
        store
            .insert_highlights(blob, grammar, version, &none)
            .unwrap();
    }
    store.retain_highlights("2").unwrap();
    assert!(!store.has_highlights("a", "rust", "1").unwrap());
    assert!(store.has_highlights("a", "rust", "2").unwrap());
    assert!(store.has_highlights("b", "toml", "2").unwrap());
}

fn changed(old: Option<&str>, new: &str) -> FileStat {
    FileStat {
        file: ChangedFile {
            path: format!("{new}.rs"),
            old_path: None,
            old_blob: old.map(Into::into),
            new_blob: Some(new.into()),
        },
        additions: Some(1),
        deletions: Some(0),
        generated: false,
        triage: Default::default(),
    }
}

fn revision(files: Vec<FileStat>) -> NewRevision {
    NewRevision {
        head: "a".repeat(40),
        base: "0".repeat(40),
        files,
        guide: None,
        interdiffs: Vec::new(),
        viewed: Vec::new(),
    }
}

#[test]
fn prunes_the_blobs_of_a_closed_item_no_open_item_has() {
    let (_dir, mut store) = store();
    store
        .insert_project(&Project {
            name: ProjectName::parse("app").unwrap(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: None,
            bots: Vec::new(),
        })
        .unwrap();
    let item = |branch: &str, files| NewItem {
        project: ProjectName::parse("app").unwrap(),
        branch: branch.into(),
        title: branch.into(),
        state: ItemState::NeedsReview,
        backend: ItemBackend::Local,
        target: "main".into(),
        revision: revision(files),
    };
    // r2 of a changes r1's, replayed to `old`, to `new`.
    let since_r1 = |files, old: &str, new: &str| NewRevision {
        interdiffs: vec![NewInterdiff {
            from: 1,
            files: vec![InterdiffFile {
                stat: changed(Some(old), new),
                change: Change::Changed,
            }],
        }],
        ..revision(files)
    };
    // app#1 has x→a, then b and x→c, r→c since r1; app#2 (open) has x→b, then
    // q→b since r1; app#3 (merged) c.
    let one = store
        .insert_item(item("one", vec![changed(Some("x"), "a")]))
        .unwrap();
    let files = vec![changed(None, "b"), changed(Some("x"), "c")];
    store
        .add_revision(
            &one.id,
            "one",
            ItemState::NeedsReview,
            since_r1(files, "r", "c"),
        )
        .unwrap();
    let two = store
        .insert_item(item("two", vec![changed(Some("x"), "b")]))
        .unwrap();
    let files = vec![changed(Some("x"), "b")];
    store
        .add_revision(
            &two.id,
            "two",
            ItemState::NeedsReview,
            since_r1(files, "q", "b"),
        )
        .unwrap();
    let three = store
        .insert_item(item("three", vec![changed(None, "c")]))
        .unwrap();
    store
        .conn
        .execute("UPDATE items SET state = 'merged' WHERE number = 3", [])
        .unwrap();
    for blob in ["a", "b", "c", "q", "r", "x", "z"] {
        store
            .insert_highlights(blob, "rust", "1", &Highlights::default())
            .unwrap();
    }

    store.prune_highlights(&one.id).unwrap();
    let kept = |store: &Store, blob| store.has_highlights(blob, "rust", "1").unwrap();
    assert!(!kept(&store, "a"));
    assert!(kept(&store, "b"), "app#2's new side");
    assert!(!kept(&store, "c"));
    assert!(kept(&store, "x"), "app#2's old side");
    assert!(!kept(&store, "r"), "app#1's replayed old side");
    assert!(kept(&store, "q"), "app#2's replayed old side");
    assert!(kept(&store, "z"), "not any item's");
    store.prune_highlights(&three.id).unwrap();
    store
        .prune_highlights(&ItemId::parse("app#9").unwrap())
        .unwrap();
    assert!(kept(&store, "b"));
}
