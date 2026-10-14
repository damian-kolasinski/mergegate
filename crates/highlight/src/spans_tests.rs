use mergegate_core::highlight::Tag;

use super::*;

fn span(start: u32, len: u32, tag: Tag) -> Span {
    Span { start, len, tag }
}

#[test]
fn cuts_runs_at_line_breaks() {
    let mut lines = Lines::default();
    lines.push("/* a\nbc */", Some(Tag::Comment));
    lines.push(" x\n", None);
    assert_eq!(
        lines.finish(),
        vec![
            vec![span(0, 4, Tag::Comment)],
            vec![span(0, 5, Tag::Comment)],
            vec![],
        ]
    );
}

#[test]
fn merges_touching_runs_of_a_tag() {
    let mut lines = Lines::default();
    lines.push("\"", Some(Tag::String));
    lines.push("ab", Some(Tag::String));
    lines.push(" ", None);
    lines.push("1", Some(Tag::String));
    lines.push("2", Some(Tag::Number));
    assert_eq!(
        lines.finish(),
        vec![vec![
            span(0, 3, Tag::String),
            span(4, 1, Tag::String),
            span(5, 1, Tag::Number),
        ]]
    );
}

#[test]
fn counts_columns_in_utf16_units() {
    let mut lines = Lines::default();
    lines.push("é😀", None);
    lines.push("x", Some(Tag::Keyword));
    assert_eq!(lines.finish(), vec![vec![span(3, 1, Tag::Keyword)]]);
}
