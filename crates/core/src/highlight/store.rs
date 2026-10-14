use super::Highlights;
use crate::item::ItemId;
use crate::store::StoreError;

/// The spans cache, keyed by blob, grammar and the installed packs'
/// version: a blob's spans under one grammar and version never change.
pub trait HighlightStore {
    fn highlights(
        &self,
        blob: &str,
        grammar: &str,
        version: &str,
    ) -> Result<Option<Highlights>, StoreError>;

    /// As `highlights`, with only `lines` (from 1, ascending) read: every
    /// other line has no spans. A diff shows a few of a large blob's lines,
    /// and reading all of them would make opening an item slow.
    fn highlight_lines(
        &self,
        blob: &str,
        grammar: &str,
        version: &str,
        lines: &[u32],
    ) -> Result<Option<Highlights>, StoreError>;

    fn has_highlights(&self, blob: &str, grammar: &str, version: &str) -> Result<bool, StoreError>;

    /// Replaces an existing entry, which holds the same spans.
    fn insert_highlights(
        &mut self,
        blob: &str,
        grammar: &str,
        version: &str,
        highlights: &Highlights,
    ) -> Result<(), StoreError>;

    /// Drops every entry under another version than `version`.
    fn retain_highlights(&mut self, version: &str) -> Result<(), StoreError>;

    /// Drops the entries of `id`'s blobs, in any revision, that no revision of
    /// an open item has.
    fn prune_highlights(&mut self, id: &ItemId) -> Result<(), StoreError>;
}
