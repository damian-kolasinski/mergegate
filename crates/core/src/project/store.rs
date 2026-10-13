use std::path::Path;

use super::{Project, ProjectName};
use crate::store::StoreError;

pub trait ProjectStore {
    /// All projects, ordered by name.
    fn projects(&self) -> Result<Vec<Project>, StoreError>;

    fn project(&self, name: &ProjectName) -> Result<Option<Project>, StoreError>;

    fn project_with_upstream(&self, upstream: &Path) -> Result<Option<Project>, StoreError>;

    fn insert_project(&mut self, project: &Project) -> Result<(), StoreError>;

    /// Replaces the settings of the project named `project.name`.
    fn update_project(&mut self, project: &Project) -> Result<(), StoreError>;
}
