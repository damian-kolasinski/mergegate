//! Projects: repositories registered with the hub.

mod add;
mod backend;
mod error;
mod find;
mod name;
mod set;
mod source;
mod store;
mod target;

use std::path::PathBuf;

use crate::github::GithubRepo;

pub use add::{AddProject, Added, add};
pub use backend::Backend;
pub use error::ProjectError;
pub use find::{at, find};
pub use name::{NameError, ProjectName};
pub use set::{Changed, KEYS, Setting, set};
pub use source::fetch_from;
pub use store::ProjectStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub name: ProjectName,
    pub backend: Backend,
    /// The branch items merge into.
    pub target: String,
    /// [`crate::git::Repo::root`] of the operator's repo.
    pub upstream: PathBuf,
    /// The repository the `github` backend publishes to. Always set while
    /// that's the backend, and kept when the project moves off it.
    pub github: Option<GithubRepo>,
    /// The GitHub logins of the bots that review the project's pull
    /// requests, which a batch of external comments waits for.
    pub bots: Vec<String>,
}
