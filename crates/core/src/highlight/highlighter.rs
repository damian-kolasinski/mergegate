use std::path::{Path, PathBuf};

use super::{Highlights, Installed, Pack};

/// The installed language packs and the engine that runs their grammars.
/// Shared by requests and the background pass, so it takes `&self`.
pub trait Highlighter {
    /// One snapshot: spans cached under its version were computed with
    /// exactly these packs.
    fn installed(&self) -> Installed;

    /// Installs the pack at `from`, or every pack in the directory `from`,
    /// replacing other versions of each. Nothing is installed unless every
    /// pack loads. Returns the packs installed.
    fn install(&self, from: &Path) -> Result<Vec<Pack>, LangError>;

    /// Returns the pack removed.
    fn remove(&self, name: &str) -> Result<Pack, LangError>;

    /// `None` when no installed pack has `grammar`, or it failed on `text`.
    fn highlight(&self, grammar: &str, text: &str) -> Option<Highlights>;
}

#[derive(Debug, thiserror::Error)]
pub enum LangError {
    #[error("{} is not a language pack: {reason}", path.display())]
    Invalid { path: PathBuf, reason: String },
    #[error("{} has no language packs", .0.display())]
    Empty(PathBuf),
    #[error("grammar {grammar} is already in the {pack} pack")]
    Taken { grammar: String, pack: String },
    #[error("no language pack named {0} is installed")]
    NotInstalled(String),
    /// Reading or writing the installed packs failed.
    #[error(transparent)]
    Failed(Box<dyn std::error::Error + Send + Sync>),
}
