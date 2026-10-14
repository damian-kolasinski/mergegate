//! Capture names to tags. tree-sitter-highlight gives a capture the longest
//! of these names whose dot-separated parts it all has, the first on a tie,
//! so `keyword.function` is a keyword and `tag.attribute` an attribute.
//! Grammars use both tree-sitter's names and nvim-treesitter's (`@text.*`,
//! `@conditional`), so both are here. A name mapped to `None` is plain text:
//! it's here so it wins over an enclosing capture, as an `@embedded` or
//! `@variable` inside a string does.
//!
//! Changing a mapping changes cached spans: bump `languages::SPANS`.

use mergegate_core::highlight::Tag;

pub(crate) const NAMES: &[(&str, Option<Tag>)] = &[
    ("keyword", Some(Tag::Keyword)),
    ("conditional", Some(Tag::Keyword)),
    ("repeat", Some(Tag::Keyword)),
    ("include", Some(Tag::Keyword)),
    ("exception", Some(Tag::Keyword)),
    ("storageclass", Some(Tag::Keyword)),
    ("attribute", Some(Tag::Attribute)),
    ("tag.attribute", Some(Tag::Attribute)),
    ("comment", Some(Tag::Comment)),
    ("constant", Some(Tag::Constant)),
    ("boolean", Some(Tag::Constant)),
    ("symbol", Some(Tag::Constant)),
    ("string.special.symbol", Some(Tag::Constant)),
    ("constructor", Some(Tag::Type)),
    ("escape", Some(Tag::Escape)),
    ("string.escape", Some(Tag::Escape)),
    ("function", Some(Tag::Function)),
    ("method", Some(Tag::Function)),
    ("label", Some(Tag::Label)),
    ("module", Some(Tag::Namespace)),
    ("namespace", Some(Tag::Namespace)),
    ("number", Some(Tag::Number)),
    ("float", Some(Tag::Number)),
    ("operator", Some(Tag::Operator)),
    ("property", Some(Tag::Property)),
    ("field", Some(Tag::Property)),
    ("variable.other.member", Some(Tag::Property)),
    ("punctuation", Some(Tag::Punctuation)),
    ("string", Some(Tag::String)),
    ("character", Some(Tag::String)),
    ("tag", Some(Tag::Tag)),
    ("type", Some(Tag::Type)),
    ("variable.builtin", Some(Tag::Variable)),
    ("text.title", Some(Tag::Heading)),
    ("markup.heading", Some(Tag::Heading)),
    ("text.emphasis", Some(Tag::Emphasis)),
    ("markup.italic", Some(Tag::Emphasis)),
    ("text.strong", Some(Tag::Strong)),
    ("markup.bold", Some(Tag::Strong)),
    ("text.uri", Some(Tag::Link)),
    ("text.reference", Some(Tag::Link)),
    ("markup.link", Some(Tag::Link)),
    ("string.special.url", Some(Tag::Link)),
    ("text.literal", Some(Tag::Literal)),
    ("markup.raw", Some(Tag::Literal)),
    ("variable", None),
    ("variable.parameter", None),
    ("embedded", None),
    ("none", None),
];

pub(crate) fn names() -> Vec<&'static str> {
    NAMES.iter().map(|(name, _)| *name).collect()
}
