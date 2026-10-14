//! Running a grammar over a text and cutting its highlights into lines.
//! Cutting them differently changes cached spans: bump `languages::SPANS`.

use mergegate_core::highlight::{Highlights, Span};
use tree_sitter_highlight::{Error, HighlightConfiguration, HighlightEvent, Highlighter};

use crate::tags::NAMES;

/// `text`'s spans under `config`, the innermost capture deciding each
/// character's tag, with `inject` finding the grammar an injection names.
pub(crate) fn highlight<'a>(
    highlighter: &'a mut Highlighter,
    config: &'a HighlightConfiguration,
    text: &'a str,
    inject: impl FnMut(&str) -> Option<&'a HighlightConfiguration> + 'a,
) -> Result<Highlights, Error> {
    let events = highlighter.highlight(config, text.as_bytes(), None, None, inject)?;
    let mut lines = Lines::default();
    let mut stack = Vec::new();
    for event in events {
        match event? {
            HighlightEvent::HighlightStart(h) => stack.push(h.0),
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let tag = stack.last().and_then(|&h| NAMES.get(h).and_then(|n| n.1));
                // Offsets fall on character boundaries of valid UTF-8; the
                // fallback only guards against a grammar that misreports one.
                let source = text.get(start..end).unwrap_or_default();
                lines.push(source, tag);
            }
        }
    }
    Ok(Highlights {
        lines: lines.finish(),
    })
}

#[derive(Default)]
struct Lines {
    done: Vec<Vec<Span>>,
    line: Vec<Span>,
    column: u32,
}

impl Lines {
    fn push(&mut self, source: &str, tag: Option<mergegate_core::highlight::Tag>) {
        let mut parts = source.split('\n');
        if let Some(first) = parts.next() {
            self.run(first, tag);
        }
        for part in parts {
            self.done.push(std::mem::take(&mut self.line));
            self.column = 0;
            self.run(part, tag);
        }
    }

    /// A run within one line, merged into the previous span when it carries
    /// on with the same tag.
    fn run(&mut self, text: &str, tag: Option<mergegate_core::highlight::Tag>) {
        let len = text.encode_utf16().count() as u32;
        if let (Some(tag), true) = (tag, len > 0) {
            match self.line.last_mut() {
                Some(last) if last.tag == tag && last.start + last.len == self.column => {
                    last.len += len;
                }
                _ => self.line.push(Span {
                    start: self.column,
                    len,
                    tag,
                }),
            }
        }
        self.column += len;
    }

    fn finish(mut self) -> Vec<Vec<Span>> {
        self.done.push(self.line);
        self.done
    }
}

#[cfg(test)]
#[path = "spans_tests.rs"]
mod tests;
