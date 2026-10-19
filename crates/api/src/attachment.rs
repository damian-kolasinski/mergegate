use serde::{Deserialize, Serialize};

/// A screenshot or recording attached to a revision.
/// `POST /api/items/{id}/attachments?name=…[&revision=N]` takes one as the
/// raw body and answers with [`Attached`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    /// The revision it was attached to.
    pub revision: u32,
    /// The file name it was attached under.
    pub name: String,
    /// As in `image/png`; read from the bytes, never from the name.
    pub media_type: String,
    pub video: bool,
    pub size: u64,
    /// Where the hub serves it, as in `/api/attachments/{sha256}`.
    pub url: String,
}

/// The attachments an item shows: the newest revision's, or, while it has
/// none, the newest earlier revision's that has some.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Demo {
    pub revision: u32,
    /// Where the hub serves them all as one zip, as in
    /// `/api/items/app%2342/demo.zip`.
    pub zip_url: String,
    /// In the order attached.
    pub attachments: Vec<Attachment>,
}

/// The query of `POST /api/items/{id}/attachments`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachTo {
    pub name: String,
    /// None attaches to the newest revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attached {
    /// As in `app#42`.
    pub item: String,
    pub attachment: Attachment,
}
