//! What the store ports have in common. Each slice defines its own store
//! trait next to its domain types, e.g. [`crate::project::ProjectStore`].

/// A store adapter's failure. The domain can't recover from one, only report
/// it.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct StoreError(Box<dyn std::error::Error + Send + Sync>);

impl StoreError {
    pub fn new(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self(source.into())
    }
}
