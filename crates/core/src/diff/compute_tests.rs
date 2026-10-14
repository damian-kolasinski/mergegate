use super::*;

/// The hunks as `git diff` prints them, to compare with its output.
fn unified(diff: &FileDiff) -> String {
    let Content::Text(hunks) = &diff.content else {
        panic!("not text: {diff:?}");
    };
    let range = |start: u32, lines: u32| match lines {
        1 => format!("{start}"),
        _ => format!("{start},{lines}"),
    };
    let mut out = String::new();
    for hunk in hunks {
        out += &format!(
            "@@ -{} +{} @@\n",
            range(hunk.old_start, hunk.old_lines),
            range(hunk.new_start, hunk.new_lines)
        );
        for line in &hunk.lines {
            let prefix = match line.kind {
                LineKind::Context => ' ',
                LineKind::Added => '+',
                LineKind::Removed => '-',
            };
            out += &format!("{prefix}{}\n", line.text);
            if !line.newline {
                out += "\\ No newline at end of file\n";
            }
        }
    }
    out
}

/// Lines 1 to 20, with the given ones replaced.
fn numbers(replace: &[(u32, &str)]) -> String {
    (1..=20)
        .map(|n| match replace.iter().find(|(at, _)| *at == n) {
            Some((_, text)) => format!("{text}\n"),
            None => format!("{n}\n"),
        })
        .collect()
}

fn diff(old: &str, new: &str) -> FileDiff {
    compute(old.as_bytes(), new.as_bytes())
}

// Expectations below are `git diff --no-index --histogram -U3` output.

#[test]
fn changes_six_lines_apart_share_a_hunk() {
    let diff = diff(&numbers(&[]), &numbers(&[(5, "five"), (12, "twelve")]));
    assert_eq!((diff.additions, diff.deletions), (Some(2), Some(2)));
    assert_eq!(
        unified(&diff),
        "@@ -2,14 +2,14 @@\n 2\n 3\n 4\n-5\n+five\n 6\n 7\n 8\n 9\n 10\n 11\n-12\n+twelve\n 13\n 14\n 15\n"
    );
}

#[test]
fn changes_seven_lines_apart_get_their_own_hunks() {
    let diff = diff(&numbers(&[]), &numbers(&[(5, "five"), (13, "thirteen")]));
    assert_eq!(
        unified(&diff),
        "@@ -2,7 +2,7 @@\n 2\n 3\n 4\n-5\n+five\n 6\n 7\n 8\n\
         @@ -10,7 +10,7 @@\n 10\n 11\n 12\n-13\n+thirteen\n 14\n 15\n 16\n"
    );
}

#[test]
fn context_stops_at_the_ends_of_the_file() {
    let diff = diff(&numbers(&[]), &numbers(&[(1, "one"), (20, "twenty")]));
    assert_eq!(
        unified(&diff),
        "@@ -1,4 +1,4 @@\n-1\n+one\n 2\n 3\n 4\n\
         @@ -17,4 +17,4 @@\n 17\n 18\n 19\n-20\n+twenty\n"
    );
}

#[test]
fn an_insertion_counts_from_the_line_before_it() {
    let old: String = (1..=10).map(|n| format!("{n}\n")).collect();
    let new = old.replace("4\n", "4\nnew\n");
    assert_eq!(
        unified(&diff(&old, &new)),
        "@@ -2,6 +2,7 @@\n 2\n 3\n 4\n+new\n 5\n 6\n 7\n"
    );
}

#[test]
fn an_added_or_deleted_file_starts_at_line_zero_on_its_empty_side() {
    let added = diff("", "x\ny\n");
    assert_eq!((added.additions, added.deletions), (Some(2), Some(0)));
    assert_eq!(unified(&added), "@@ -0,0 +1,2 @@\n+x\n+y\n");
    assert_eq!(unified(&diff("x\ny\n", "")), "@@ -1,2 +0,0 @@\n-x\n-y\n");
}

#[test]
fn a_missing_final_newline_is_a_change() {
    assert_eq!(
        unified(&diff("a\nb", "a\nb\nc\n")),
        "@@ -1,2 +1,3 @@\n a\n-b\n\\ No newline at end of file\n+b\n+c\n"
    );
}

#[test]
fn identical_blobs_have_no_hunks() {
    assert_eq!(
        diff("a\n", "a\n"),
        FileDiff {
            additions: Some(0),
            deletions: Some(0),
            content: Content::Text(Vec::new()),
        }
    );
}

#[test]
fn keeps_carriage_returns_and_replaces_bytes_that_arent_utf8() {
    let diff = compute(b"a\r\n", b"\xffb\n");
    assert_eq!(unified(&diff), "@@ -1 +1 @@\n-a\r\n+\u{fffd}b\n");
}

#[test]
fn a_nul_byte_near_the_start_makes_a_blob_binary() {
    assert_eq!(compute(b"a\n", b"\0\x01"), FileDiff::binary());
    assert_eq!(compute(b"\0", b""), FileDiff::binary());
    let late_nul = [&[b'a'; BINARY_PROBE][..], b"\0"].concat();
    assert!(matches!(compute(b"", &late_nul).content, Content::Text(_)));
}

#[test]
fn too_many_changed_lines_are_counted_but_not_kept() {
    let big: String = (0..=MAX_CHANGED_LINES).map(|n| format!("{n}\n")).collect();
    assert_eq!(
        diff("", &big),
        FileDiff {
            additions: Some(MAX_CHANGED_LINES + 1),
            deletions: Some(0),
            content: Content::TooLarge,
        }
    );
    let at_limit: String = (1..=MAX_CHANGED_LINES).map(|n| format!("{n}\n")).collect();
    assert!(matches!(diff("", &at_limit).content, Content::Text(_)));
}

#[test]
fn context_is_configurable() {
    let old = numbers(&[]);
    let new = numbers(&[(5, "five")]);
    let input = InternedInput::new(old.as_bytes(), new.as_bytes());
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    let hunks = hunks(&input, &diff, 0);
    assert_eq!(
        (hunks.len(), hunks[0].old_start, hunks[0].lines.len()),
        (1, 5, 2)
    );
}
