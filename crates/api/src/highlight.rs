use serde::{Deserialize, Serialize};

/// A highlighted run within a line: start and length in UTF-16 code units,
/// as the browser counts them, and what the run is.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span(pub u32, pub u32, pub Tag);

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// `GET /api/highlights/{grammar}/{blob}`: a whole blob's spans, for the
/// lines the UI shows beyond a diff's hunks. Not found until the hub has
/// computed them.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobHighlights {
    /// One list per line, the first line's first.
    pub lines: Vec<Vec<Span>>,
}
