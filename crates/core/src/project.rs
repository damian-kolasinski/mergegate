//! Projects: repositories registered with the hub.

mod add;
mod backend;
mod error;
mod find;
mod name;
mod set;
mod store;
mod target;

use std::path::PathBuf;

pub use add::{AddProject, Added, add};
pub use backend::Backend;
pub use error::ProjectError;
pub use find::{at, find};
pub use name::{NameError, ProjectName};
pub use set::{KEYS, Setting, set};
pub use store::ProjectStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub name: ProjectName,
    pub backend: Backend,
    /// The branch items merge into.
    pub target: String,
    /// [`crate::git::Repo::root`] of the operator's repo.
    pub upstream: PathBuf,
}
