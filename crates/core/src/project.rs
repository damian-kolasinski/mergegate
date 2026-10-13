//! Projects: repositories registered with the hub.

mod add;
mod error;
mod find;
mod name;
mod set;
mod store;
mod target;

use std::path::PathBuf;

pub use add::{AddProject, Added, add};
pub use api::project::Backend;
pub use error::ProjectError;
pub use find::{at, find};
pub use name::{NameError, ProjectName};
pub use set::{Changed, KEYS, Setting, set};
pub use store::ProjectStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub name: ProjectName,
    pub backend: Backend,
    pub target: String,
    /// [`crate::git::Repo::root`] of the operator's repo.
    pub upstream: PathBuf,
}

impl Project {
    pub fn to_api(&self) -> api::project::Project {
        api::project::Project {
            name: self.name.to_string(),
            backend: self.backend,
            target: self.target.clone(),
            upstream: self.upstream.display().to_string(),
        }
    }
}
