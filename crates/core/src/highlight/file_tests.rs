use super::*;
use crate::diff::Line;
use crate::fake::{FakeStore, pack};
use crate::highlight::Tag;

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
fn reads_both_sides_under_the_installed_grammar_version() {
    let mut store = FakeStore::default();
    let old = numbered(2, Tag::String);
    store.insert_highlights("old", "rs", "2", &old).unwrap();
    store
        .insert_highlights("new", "rs", "1", &numbered(2, Tag::Comment))
        .unwrap();
    let found = file_highlights(&store, &[pack("rs", "2")], &[], &file("a.rs")).unwrap();
    assert_eq!(found.grammar.as_deref(), Some("rs"));
    assert_eq!(found.old, Some(old));
    assert_eq!(found.new, None, "spans of another version don't count");
}

#[test]
fn names_the_known_pack_for_a_file_no_installed_one_covers() {
    let store = FakeStore::default();
    let known = [pack("md", "9"), pack("rs", "9")];
    let found = file_highlights(&store, &[pack("rs", "1")], &known, &file("a.md")).unwrap();
    assert_eq!(
        found,
        FileHighlights {
            missing: Some("md".into()),
            ..FileHighlights::default()
        }
    );
    let found = file_highlights(&store, &[], &known, &file("a.txt")).unwrap();
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
    let line = |kind| Line {
        kind,
        text: String::new(),
        newline: true,
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
fn reads_a_blobs_spans_under_the_installed_version_of_its_grammar() {
    let mut store = FakeStore::default();
    let spans = numbered(1, Tag::Type);
    store.insert_highlights("b", "rs", "2", &spans).unwrap();
    let installed = [pack("rs", "2")];
    assert_eq!(
        blob_highlights(&store, &installed, "rs", "b").unwrap(),
        Some(spans)
    );
    assert_eq!(
        blob_highlights(&store, &installed, "md", "b").unwrap(),
        None
    );
    assert_eq!(
        blob_highlights(&store, &[pack("rs", "1")], "rs", "b").unwrap(),
        None
    );
}
