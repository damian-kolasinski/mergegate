use crate::github::{GithubRepo, Pull};
use crate::project::Backend;

/// The backend an item lands through, and what the item has there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemBackend {
    /// The repository the item was opened on, and its pull request there,
    /// opened or adopted by its first submit.
    Github {
        repo: GithubRepo,
        pull: Pull,
    },
    Local,
}

impl ItemBackend {
    pub fn kind(&self) -> Backend {
        match self {
            Self::Github { .. } => Backend::Github,
            Self::Local => Backend::Local,
        }
    }
}
