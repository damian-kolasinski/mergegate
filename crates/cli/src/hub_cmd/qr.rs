//! QR codes drawn in a terminal, two rows of modules to a line.

use qrcode::{Color, QrCode};

/// Light modules around the code. Scanners need a margin; the standard's four
/// cost lines a terminal can't spare, and phone cameras read two.
const QUIET: usize = 2;

/// Black on white whatever the terminal's theme: many scanners don't read a
/// code with its colors inverted, as a dark theme would draw it.
const COLORS: &str = "\x1b[38;5;16;48;5;231m";
const RESET: &str = "\x1b[0m";

/// `text` as a QR code, one line per two rows of modules, without a trailing
/// newline.
pub(super) fn render(text: &str) -> String {
    let code = QrCode::new(text.as_bytes()).expect("a hub URL fits in a QR code");
    let width = code.width();
    let modules = code.to_colors();
    let size = width + 2 * QUIET;
    let dark = |x: usize, y: usize| {
        (QUIET..QUIET + width).contains(&x)
            && (QUIET..QUIET + width).contains(&y)
            && modules[(y - QUIET) * width + x - QUIET] == Color::Dark
    };
    (0..size)
        .step_by(2)
        .map(|y| {
            let cells: String = (0..size)
                .map(|x| match (dark(x, y), dark(x, y + 1)) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect();
            format!("{COLORS}{cells}{RESET}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "qr_tests.rs"]
mod tests;
