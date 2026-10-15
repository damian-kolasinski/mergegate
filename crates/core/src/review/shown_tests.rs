use super::*;

#[test]
fn a_comment_is_on_lines_one_hunk_shows() {
    let hunk = |old_start, old_lines, new_start, new_lines| Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines: Vec::new(),
    };
    let hunks = [hunk(1, 4, 1, 5), hunk(20, 0, 21, 2)];
    assert!(shown(&hunks, Side::New, 1, 5));
    assert!(shown(&hunks, Side::Old, 2, 4));
    assert!(shown(&hunks, Side::New, 21, 22));
    // Between the hunks, across them, and on a side a hunk has no lines of.
    assert!(!shown(&hunks, Side::New, 6, 6));
    assert!(!shown(&hunks, Side::New, 5, 21));
    assert!(!shown(&hunks, Side::Old, 20, 20));
    assert!(!shown(&hunks, Side::New, 3, 2));
}
