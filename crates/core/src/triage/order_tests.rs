use super::*;

fn rank(path: &str, importance: Option<u8>, hint: Option<Hint>) -> Rank<'_> {
    Rank {
        path,
        importance,
        hint,
    }
}

/// The paths in order, each marked `^` when the judge raised it.
fn ordered(files: &[Rank], guide: &str, by: OrderBy) -> Vec<String> {
    order(files, &Guide::parse(guide), by)
        .into_iter()
        .map(|p| {
            let mark = if p.raised { "^" } else { "" };
            format!("{}{mark}", files[p.index].path)
        })
        .collect()
}

const GUIDE: &str = "## Files\n- `b.rs` — first\n- `a.rs` — second\n";

#[test]
fn orders_files() {
    let cases: &[(&str, &[Rank], &str, &[&str])] = &[
        (
            "with nothing to go by, path order",
            &[rank("b.rs", None, None), rank("a.rs", None, None)],
            "",
            &["a.rs", "b.rs"],
        ),
        (
            "heuristics sink tests and raise migrations",
            &[
                rank("a_tests.rs", None, Some(Hint::Tests)),
                rank("b.rs", None, None),
                rank("c.sql", None, Some(Hint::Migration)),
            ],
            "",
            &["c.sql", "b.rs", "a_tests.rs"],
        ),
        (
            "the guide's order comes first",
            &[
                rank("a.rs", None, None),
                rank("b.rs", None, None),
                rank("c.sql", None, Some(Hint::Migration)),
            ],
            GUIDE,
            &["b.rs", "a.rs", "c.sql"],
        ),
        (
            "the judge orders what the guide leaves out, ahead of heuristics",
            &[
                rank("a.rs", Some(10), None),
                rank("b.rs", Some(20), None),
                rank("c.rs", Some(70), None),
                rank("d.sql", Some(30), Some(Hint::Migration)),
            ],
            GUIDE,
            &["b.rs", "a.rs", "c.rs", "d.sql"],
        ),
        (
            "a file the judge skipped counts as an average one",
            &[
                rank("a.rs", Some(70), None),
                rank("b.rs", Some(30), None),
                rank("c.rs", None, None),
                rank("d.sql", None, Some(Hint::Migration)),
            ],
            "",
            &["a.rs", "d.sql", "c.rs", "b.rs"],
        ),
        (
            "heuristics break the judge's ties",
            &[
                rank("a.rs", Some(50), None),
                rank("b.rs", Some(50), Some(Hint::Signature)),
                rank("c.md", Some(50), Some(Hint::Docs)),
            ],
            "",
            &["b.rs", "a.rs", "c.md"],
        ),
        (
            "a score of 80 or more goes ahead of the guide",
            &[
                rank("a.rs", Some(30), None),
                rank("b.rs", Some(20), None),
                rank("c.rs", Some(80), None),
                rank("d.rs", Some(79), None),
            ],
            GUIDE,
            &["c.rs^", "b.rs", "a.rs", "d.rs"],
        ),
        (
            "the files it raises go highest first",
            &[
                rank("a.rs", Some(90), None),
                rank("b.rs", Some(85), None),
                rank("c.rs", Some(99), None),
            ],
            GUIDE,
            &["c.rs^", "a.rs", "b.rs"],
        ),
        (
            "a file the guide leads with isn't raised",
            &[rank("a.rs", Some(10), None), rank("b.rs", Some(95), None)],
            GUIDE,
            &["b.rs", "a.rs"],
        ),
        (
            "a high score is only raised past what it outranks",
            &[
                rank("a.rs", Some(95), None),
                rank("b.rs", Some(20), None),
                rank("c.rs", Some(90), None),
            ],
            "## Files\n- `b.rs`\n- `a.rs`\n",
            &["a.rs^", "c.rs^", "b.rs"],
        ),
    ];
    for &(name, files, guide, want) in cases {
        assert_eq!(ordered(files, guide, OrderBy::Guide), want, "{name}");
    }
}

#[test]
fn orders_files_by_the_judge_alone() {
    let cases: &[(&str, &[Rank], &str, &[&str])] = &[
        (
            "its scores go ahead of the guide's list, and nothing is raised",
            &[
                rank("a.rs", Some(30), None),
                rank("b.rs", Some(20), None),
                rank("c.rs", Some(60), None),
                rank("d.rs", Some(90), None),
            ],
            GUIDE,
            &["d.rs", "c.rs", "a.rs", "b.rs"],
        ),
        (
            "the guide breaks its ties, ahead of the heuristics",
            &[
                rank("a.rs", Some(40), None),
                rank("b.rs", Some(40), None),
                rank("c.sql", Some(40), Some(Hint::Migration)),
            ],
            GUIDE,
            &["b.rs", "a.rs", "c.sql"],
        ),
        (
            "a file it skipped counts as an average one",
            &[
                rank("a.rs", Some(40), None),
                rank("b.rs", None, None),
                rank("c.rs", Some(60), None),
            ],
            GUIDE,
            &["c.rs", "b.rs", "a.rs"],
        ),
        (
            "before it scores anything, the guide's order",
            &[
                rank("a.rs", None, None),
                rank("b.rs", None, None),
                rank("c.sql", None, Some(Hint::Migration)),
            ],
            GUIDE,
            &["b.rs", "a.rs", "c.sql"],
        ),
    ];
    for &(name, files, guide, want) in cases {
        assert_eq!(ordered(files, guide, OrderBy::Judge), want, "{name}");
    }
}

#[test]
fn names_each_order() {
    for by in [OrderBy::Guide, OrderBy::Judge] {
        assert_eq!(OrderBy::parse(by.as_str()), Some(by));
    }
    assert_eq!(OrderBy::parse("path"), None);
}
