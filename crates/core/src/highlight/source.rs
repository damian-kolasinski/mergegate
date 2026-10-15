use std::path::Path;

use super::LangError;

/// A pack archive released with this mg, which it downloads by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub name: String,
    pub version: String,
    /// The archive's SHA-256, hex: what the download must match, and what
    /// an installed pack's [`super::Installed::digests`] are compared with.
    pub sha256: String,
}

/// Where the packs released with this mg come from.
pub trait PackSource {
    /// By name. Empty when this mg was built without its release's index,
    /// as dev builds are: those install packs only from files.
    fn releases(&self) -> &[Release];

    /// Downloads `release`'s archive into the directory `into`, checked
    /// against its digest.
    fn download(&self, release: &Release, into: &Path) -> Result<(), LangError>;
}
