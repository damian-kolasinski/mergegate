use super::{Rule, Tier};
use crate::project::ProjectName;

/// The operator's hand-editable rules file of each project.
pub trait RulesFile {
    /// The file as written; none when the project has no file.
    fn read(&self, project: &ProjectName) -> Result<Option<RulesText>, RulesFileError>;

    /// Replaces the file's text, creating it when there's none.
    fn write(&self, project: &ProjectName, text: &str) -> Result<(), RulesFileError>;

    /// `text` with `rule` added first or last and the rest, comments too,
    /// kept as written.
    fn insert(&self, text: &str, rule: &Rule, at: Place) -> Result<String, RulesFileError>;

    /// Where the file is, for messages.
    fn location(&self, project: &ProjectName) -> String;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesText {
    pub text: String,
    /// In file order.
    pub rules: Vec<Rule>,
}

/// Where a rule the hub adds goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    First,
    Last,
}

impl Place {
    /// A `review` rule first, where it overrides every other; a `trusted`
    /// one last, where it can't override any.
    pub fn of(tier: Tier) -> Self {
        match tier {
            Tier::Review => Self::First,
            Tier::Trusted => Self::Last,
        }
    }

    pub fn insert(self, rules: &mut Vec<Rule>, rule: Rule) {
        match self {
            Self::First => rules.insert(0, rule),
            Self::Last => rules.push(rule),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RulesFileError {
    /// The file isn't valid TOML or doesn't hold rules.
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Io(String),
}
