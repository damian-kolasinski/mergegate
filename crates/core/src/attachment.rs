//! Demo attachments: the screenshots and recordings an agent adds to a
//! revision, for the operator to watch before reading the diff.
//! A closed item keeps them for the hub's `attachment-ttl`.

mod attach;
mod demo;
mod error;
mod media;
mod prune;
mod store;

pub use attach::{Attach, attach};
pub use demo::{Demo, choose, demo};
pub use error::AttachmentError;
pub use media::{MAX_SIZE, Media, SNIFF_LEN, valid_name};
pub use prune::prune;
pub use store::{AttachmentStore, NewAttachment};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub revision: u32,
    /// The file's name as the agent attached it, for the operator only.
    pub name: String,
    pub media: Media,
    pub size: u64,
    /// Lowercase hex; the stored file and its URL are named by it.
    pub sha256: String,
    /// Of the bytes, for a zip of the demo; None for one attached before
    /// the store kept it.
    pub crc32: Option<u32>,
}
