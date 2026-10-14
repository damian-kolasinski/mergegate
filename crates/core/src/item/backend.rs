use crate::github::Pull;
use crate::project::Backend;

/// The backend an item lands through, and what the item has there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemBackend {
    /// Opened or adopted by the item's first submit.
    Github {
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
