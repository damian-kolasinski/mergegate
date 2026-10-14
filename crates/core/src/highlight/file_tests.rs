use super::*;
use crate::diff::Line;
use crate::fake::{FakeStore, installed, pack};
use crate::highlight::Tag;

fn line(kind: LineKind) -> Line {
    Line {
        kind,
        text: String::new(),
        newline: true,
    }
}

/// Context line 1, line 2 replaced, context line 3.
fn hunk() -> Hunk {
    Hunk {
        old_start: 1,
        old_lines: 3,
        new_start: 1,
        new_lines: 3,
        lines: vec![
            line(LineKind::Context),
            line(LineKind::Removed),
            line(LineKind::Added),
            line(LineKind::Context),
        ],
    }
}

fn file(path: &str) -> ChangedFile {
    ChangedFile {
        path: path.into(),
        old_path: None,
        old_blob: Some("old".into()),
        new_blob: Some("new".into()),
    }
}

/// Line `n` of `lines` lines has one span of length `n`.
fn numbered(lines: u32, tag: Tag) -> Highlights {
    Highlights {
        lines: (1..=lines)
            .map(|len| vec![Span { start: 0, len, tag }])
            .collect(),
    }
}

#[test]
fn reads_both_sides_under_the_installed_packs_version() {
    let mut store = FakeStore::default();
    store
        .insert_highlights("old", "rs", "rs@2", &numbered(3, Tag::String))
        .unwrap();
    store
        .insert_highlights("new", "rs", "rs@1", &numbered(3, Tag::Comment))
        .unwrap();
    let rs = installed(&[("rs", "2")]);
    let found = file_highlights(&store, &rs, &[], &file("a.rs"), &[hunk()]).unwrap();
    assert_eq!(found.grammar.as_deref(), Some("rs"));
    assert!(found.old.is_some());
    assert_eq!(found.new, None, "spans of another version don't count");
}

#[test]
fn reads_only_the_lines_the_hunks_show() {
    let mut store = FakeStore::default();
    store
        .insert_highlights("old", "rs", "rs@1", &numbered(4, Tag::String))
        .unwrap();
    store
        .insert_highlights("new", "rs", "rs@1", &numbered(4, Tag::Comment))
        .unwrap();
    let rs = installed(&[("rs", "1")]);
    let found = file_highlights(&store, &rs, &[], &file("a.rs"), &[hunk()]).unwrap();
    let lens = |h: Option<Highlights>| -> Vec<Option<u32>> {
        h.unwrap()
            .lines
            .iter()
            .map(|l| l.first().map(|s| s.len))
            .collect()
    };
    assert_eq!(lens(found.old), [None, Some(2)]);
    assert_eq!(lens(found.new), [Some(1), Some(2), Some(3)]);
    let spans: Vec<Tag> = hunk_spans(&hunk(), &found_again(&store, &rs))
        .into_iter()
        .map(|s| s[0].tag)
        .collect();
    assert_eq!(
        spans,
        [Tag::Comment, Tag::String, Tag::Comment, Tag::Comment]
    );
}

#[test]
fn shows_each_sides_lines_across_hunks() {
    let later = Hunk {
        old_start: 10,
        old_lines: 4,
        new_start: 10,
        new_lines: 3,
        lines: vec![
            line(LineKind::Context),
            line(LineKind::Removed),
            line(LineKind::Removed),
            line(LineKind::Added),
            line(LineKind::Context),
        ],
    };
    assert_eq!(
        shown(&[hunk(), later]),
        (vec![2, 11, 12], vec![1, 2, 3, 10, 11, 12])
    );
    let added = Hunk {
        old_start: 0,
        old_lines: 0,
        new_start: 1,
        new_lines: 1,
        lines: vec![line(LineKind::Added)],
    };
    assert_eq!(shown(&[added]), (vec![], vec![1]));
    let deleted = Hunk {
        old_start: 1,
        old_lines: 1,
        new_start: 0,
        new_lines: 0,
        lines: vec![line(LineKind::Removed)],
    };
    assert_eq!(shown(&[deleted]), (vec![1], vec![]));
}

fn found_again(store: &FakeStore, rs: &Installed) -> FileHighlights {
    file_highlights(store, rs, &[], &file("a.rs"), &[hunk()]).unwrap()
}

#[test]
fn reads_no_side_without_hunks() {
    let mut store = FakeStore::default();
    store
        .insert_highlights("new", "rs", "rs@1", &numbered(1, Tag::Comment))
        .unwrap();
    let rs = installed(&[("rs", "1")]);
    let found = file_highlights(&store, &rs, &[], &file("a.rs"), &[]).unwrap();
    assert_eq!((found.old, found.new), (None, None));
}

#[test]
fn names_the_known_pack_for_a_file_no_installed_one_covers() {
    let store = FakeStore::default();
    let known = [pack("md", "9"), pack("rs", "9")];
    let rs = installed(&[("rs", "1")]);
    let found = file_highlights(&store, &rs, &known, &file("a.md"), &[hunk()]).unwrap();
    assert_eq!(
        found,
        FileHighlights {
            missing: Some("md".into()),
            ..FileHighlights::default()
        }
    );
    let none = installed(&[]);
    let found = file_highlights(&store, &none, &known, &file("a.txt"), &[hunk()]).unwrap();
    assert_eq!(found, FileHighlights::default());
}

#[test]
fn gives_a_removed_line_the_old_sides_spans_and_others_the_new_sides() {
    let highlights = FileHighlights {
        grammar: Some("rs".into()),
        missing: None,
        old: Some(numbered(5, Tag::String)),
        new: Some(numbered(5, Tag::Comment)),
    };
    let hunk = Hunk {
        old_start: 2,
        old_lines: 3,
        new_start: 3,
        new_lines: 2,
        lines: vec![
            line(LineKind::Context),
            line(LineKind::Removed),
            line(LineKind::Removed),
            line(LineKind::Added),
        ],
    };
    let spans: Vec<(u32, Tag)> = hunk_spans(&hunk, &highlights)
        .into_iter()
        .map(|s| (s[0].len, s[0].tag))
        .collect();
    assert_eq!(
        spans,
        [
            (3, Tag::Comment),
            (3, Tag::String),
            (4, Tag::String),
            (4, Tag::Comment),
        ]
    );
}

#[test]
fn gives_lines_without_spans_none() {
    let hunk = Hunk {
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        lines: vec![Line {
            kind: LineKind::Removed,
            text: "x".into(),
            newline: true,
        }],
    };
    let none = FileHighlights::default();
    assert_eq!(hunk_spans(&hunk, &none), [&[] as &[Span]]);
}

#[test]
fn reads_a_blobs_spans_under_the_installed_packs_version() {
    let mut store = FakeStore::default();
    let spans = numbered(1, Tag::Type);
    store.insert_highlights("b", "rs", "rs@2", &spans).unwrap();
    let now = installed(&[("rs", "2")]);
    assert_eq!(
        blob_highlights(&store, &now, "rs", "b").unwrap(),
        Some(spans)
    );
    assert_eq!(blob_highlights(&store, &now, "md", "b").unwrap(), None);
    assert_eq!(
        blob_highlights(&store, &installed(&[("rs", "1")]), "rs", "b").unwrap(),
        None
    );
}
