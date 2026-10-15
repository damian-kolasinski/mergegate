//! Syntax highlighting with the language packs the operator installs: spans
//! are computed for open items' files in the background, cached per blob and
//! grammar, and read with an item's diff (tech-stack §6).

mod detect;
mod file;
mod grammar;
mod highlighter;
mod pass;
mod released;
mod source;
mod store;

use std::collections::BTreeMap;

pub use detect::{Detected, detect};
pub use file::{FileHighlights, blob_highlights, file_highlights, hunk_spans};
pub use grammar::grammar_for;
pub use highlighter::{Highlighter, LangError};
pub use pass::{MAX_BYTES, Pending, compute, pending};
pub use released::{available, install, outdated, resolve};
pub use source::{PackSource, Release};
pub use store::HighlightStore;

/// The installed packs, read together with the version the spans cache keys
/// them by. Injections reach across packs, so a blob's spans under one
/// grammar depend on every installed pack: the version changes whenever a
/// pack is added, removed or reinstalled, or mg cuts spans differently.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    /// By name.
    pub packs: Vec<Pack>,
    pub version: String,
    /// The SHA-256, hex, of the archive each pack was installed from, by
    /// pack name.
    pub digests: BTreeMap<String, String>,
}

/// An installed language pack, or one mg knows how to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    pub name: String,
    pub version: String,
    pub grammars: Vec<Grammar>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grammar {
    /// Unique across installed packs: injections name the grammar they want.
    pub name: String,
    /// Without the dot, matched regardless of case.
    pub extensions: Vec<String>,
    /// Whole file names, matched before extensions.
    pub filenames: Vec<String>,
    /// Other names an injection may use, as a Markdown fence's info string.
    pub aliases: Vec<String>,
}

/// A file's spans, one list per line, each in column order. A file that
/// couldn't be highlighted has no lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Highlights {
    pub lines: Vec<Vec<Span>>,
}

impl Highlights {
    /// Line `number`'s spans, counting from 1; none past the end.
    pub fn line(&self, number: u32) -> &[Span] {
        let index = (number as usize).wrapping_sub(1);
        self.lines.get(index).map_or(&[], Vec::as_slice)
    }
}

/// Columns are UTF-16 code units, as the browser counts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub len: u32,
    pub tag: Tag,
}

/// What a span is, coarse enough for every grammar's captures to map onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tag {
    Attribute,
    Comment,
    Constant,
    Emphasis,
    Escape,
    Function,
    Heading,
    Keyword,
    Label,
    Link,
    Literal,
    Namespace,
    Number,
    Operator,
    Property,
    Punctuation,
    String,
    Strong,
    Tag,
    Type,
    Variable,
}

impl Tag {
    pub const ALL: [Tag; 21] = [
        Tag::Attribute,
        Tag::Comment,
        Tag::Constant,
        Tag::Emphasis,
        Tag::Escape,
        Tag::Function,
        Tag::Heading,
        Tag::Keyword,
        Tag::Label,
        Tag::Link,
        Tag::Literal,
        Tag::Namespace,
        Tag::Number,
        Tag::Operator,
        Tag::Property,
        Tag::Punctuation,
        Tag::String,
        Tag::Strong,
        Tag::Tag,
        Tag::Type,
        Tag::Variable,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tag::Attribute => "attribute",
            Tag::Comment => "comment",
            Tag::Constant => "constant",
            Tag::Emphasis => "emphasis",
            Tag::Escape => "escape",
            Tag::Function => "function",
            Tag::Heading => "heading",
            Tag::Keyword => "keyword",
            Tag::Label => "label",
            Tag::Link => "link",
            Tag::Literal => "literal",
            Tag::Namespace => "namespace",
            Tag::Number => "number",
            Tag::Operator => "operator",
            Tag::Property => "property",
            Tag::Punctuation => "punctuation",
            Tag::String => "string",
            Tag::Strong => "strong",
            Tag::Tag => "tag",
            Tag::Type => "type",
            Tag::Variable => "variable",
        }
    }

    pub fn parse(name: &str) -> Option<Tag> {
        Tag::ALL.into_iter().find(|t| t.name() == name)
    }
}
