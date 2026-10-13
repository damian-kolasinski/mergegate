use std::fmt;

use crate::project::{NameError, ProjectName};

/// An item's ID: its project's name and its number there, as in `app#42`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId {
    project: ProjectName,
    number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdError {
    #[error("{0:?} is not an item ID; IDs look like app#42")]
    Malformed(String),
    #[error(transparent)]
    InvalidProject(#[from] NameError),
}

impl ItemId {
    pub fn new(project: ProjectName, number: u64) -> Self {
        Self { project, number }
    }

    pub fn parse(id: &str) -> Result<Self, IdError> {
        let malformed = || IdError::Malformed(id.into());
        let (project, number) = id.split_once('#').ok_or_else(malformed)?;
        let number = match number.parse() {
            Ok(n) if n > 0 && !number.starts_with(['+', '0']) => n,
            _ => return Err(malformed()),
        };
        Ok(Self::new(ProjectName::parse(project)?, number))
    }

    pub fn project(&self) -> &ProjectName {
        &self.project
    }

    pub fn number(&self) -> u64 {
        self.number
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.project, self.number)
    }
}

#[cfg(test)]
#[path = "id_tests.rs"]
mod tests;
