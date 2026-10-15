use super::*;
use crate::diff::{Content, compute};

const CASES: &str = include_str!("lines_cases.txt");

struct Case<'a> {
    name: &'a str,
    old: &'a str,
    new: &'a str,
    lines: (u32, u32),
    want: Mapped,
}

fn cases() -> Vec<Case<'static>> {
    let mut lines = CASES
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty());
    let mut cases = Vec::new();
    while let Some(name) = lines.next() {
        let mut field = |key: &str| {
            let line = lines.next().unwrap_or_else(|| panic!("{name}: no {key}"));
            line.strip_prefix(key)
                .and_then(|rest| rest.strip_prefix(':'))
                .unwrap_or_else(|| panic!("{name}: {line:?} isn't {key}"))
                .trim()
        };
        let (old, new) = (field("old"), field("new"));
        let range = |text: &str| {
            let (a, b) = text.split_once('-').unwrap();
            (a.parse().unwrap(), b.parse().unwrap())
        };
        let lines = range(field("lines"));
        let want = match field("want").split_once(' ') {
            None => Mapped::Gone,
            Some(("same", at)) => {
                let (start_line, line) = range(at);
                Mapped::Same { start_line, line }
            }
            Some(("changed", at)) => {
                let (start_line, line) = range(at);
                Mapped::Changed { start_line, line }
            }
            Some(other) => panic!("{name}: no such want {other:?}"),
        };
        cases.push(Case {
            name,
            old,
            new,
            lines,
            want,
        });
    }
    cases
}

/// A file with `words` as its lines.
fn file(words: &str) -> String {
    words.split(' ').map(|word| format!("{word}\n")).collect()
}

#[test]
fn maps_ranges_through_a_computed_diff() {
    let cases = cases();
    assert!(cases.len() > 10, "lines_cases.txt didn't parse");
    for case in cases {
        let diff = compute(file(case.old).as_bytes(), file(case.new).as_bytes());
        let Content::Text(hunks) = diff.content else {
            panic!("{}: not text", case.name);
        };
        let (start_line, line) = case.lines;
        assert_eq!(map(&hunks, start_line, line), case.want, "{}", case.name);
    }
}
