use qrcode::{Color, QrCode};

use super::{COLORS, QUIET, RESET, render};

/// The modules a rendering draws, read back from its characters.
fn modules(rendered: &str) -> Vec<Vec<bool>> {
    let mut rows = Vec::new();
    for line in rendered.lines() {
        let cells = line
            .strip_prefix(COLORS)
            .and_then(|l| l.strip_suffix(RESET))
            .unwrap_or_else(|| panic!("an uncolored line: {line:?}"));
        let (top, bottom): (Vec<bool>, Vec<bool>) = cells
            .chars()
            .map(|c| match c {
                '█' => (true, true),
                '▀' => (true, false),
                '▄' => (false, true),
                ' ' => (false, false),
                other => panic!("unexpected {other:?}"),
            })
            .unzip();
        rows.push(top);
        rows.push(bottom);
    }
    rows
}

#[test]
fn draws_every_module_of_the_code_inside_a_light_margin() {
    let url = "http://mac-mini-appunite:7800/#/";
    let code = QrCode::new(url.as_bytes()).unwrap();
    let width = code.width();
    let expected = code.to_colors();

    let drawn = modules(&render(url));

    let size = width + 2 * QUIET;
    // An odd size leaves the last line's bottom half as margin.
    assert_eq!(drawn.len(), size.next_multiple_of(2));
    for (y, row) in drawn.iter().enumerate() {
        assert_eq!(row.len(), size, "row {y}");
        for (x, &dark) in row.iter().enumerate() {
            let inside = (QUIET..QUIET + width).contains(&x) && (QUIET..QUIET + width).contains(&y);
            let want = inside && expected[(y - QUIET) * width + x - QUIET] == Color::Dark;
            assert_eq!(dark, want, "module ({x}, {y})");
        }
    }
}

#[test]
fn ends_without_a_newline() {
    assert!(!render("http://hub:7800/#/").ends_with('\n'));
}
