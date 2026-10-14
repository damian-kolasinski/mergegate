use super::Highlights;
use crate::item::ItemId;
use crate::store::StoreError;

/// The spans cache, keyed by blob, grammar and the version of the pack that
/// has the grammar: a blob's spans under one grammar version never change.
pub trait HighlightStore {
    fn highlights(
        &self,
        blob: &str,
        grammar: &str,
        version: &str,
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

    /// Drops every entry whose grammar and version aren't in `installed`.
    fn retain_highlights(&mut self, installed: &[(&str, &str)]) -> Result<(), StoreError>;

    /// Drops the entries of `id`'s blobs, in any revision, that no revision of
    /// an open item has.
    fn prune_highlights(&mut self, id: &ItemId) -> Result<(), StoreError>;
}
