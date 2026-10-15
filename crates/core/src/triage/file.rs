use super::Rule;
use crate::project::ProjectName;

/// The operator's hand-editable rules file of each project.
pub trait RulesFile {
    /// The rules in file order; none when the project has no file.
    fn read(&self, project: &ProjectName) -> Result<Vec<Rule>, RulesFileError>;

    /// Where the file is, for messages.
    fn location(&self, project: &ProjectName) -> String;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RulesFileError {
    /// The file isn't valid TOML or doesn't hold rules.
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Unreadable(String),
}
