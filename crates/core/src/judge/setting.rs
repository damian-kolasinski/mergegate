use super::JudgeStore;
use crate::project::{ProjectName, ProjectStore};
use crate::store::StoreError;

/// Pinned, so a provider's upgrade doesn't silently reorder reviews.
pub const DEFAULT_MODEL: &str = "jev-1.13.0";

/// The confidence from which the judge promotes a trusted file.
pub const DEFAULT_THRESHOLD: f64 = 0.8;

#[derive(Debug, Clone, PartialEq)]
pub struct JudgeSetting {
    pub provider: Provider,
    /// The promotion confidence, in (0, 1].
    pub threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Jev {
        model: String,
    },
    /// Run by `sh -c`.
    Command {
        command: String,
    },
}

impl Provider {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Jev { .. } => "jev",
            Self::Command { .. } => "command",
        }
    }
}

impl JudgeSetting {
    /// What the verdicts are cached under: another model or command judges
    /// again, another threshold doesn't, since promotion is applied on read.
    pub fn key(&self) -> String {
        match &self.provider {
            Provider::Jev { model } => format!("jev {model}"),
            Provider::Command { command } => format!("command {command}"),
        }
    }

    /// `mg judge set`'s arguments; `none` is no judge.
    pub fn parse(
        provider: &str,
        model: Option<&str>,
        command: Option<&str>,
        threshold: Option<f64>,
    ) -> Result<Option<Self>, SettingError> {
        let provider = match (provider, model, command) {
            ("none", None, None) if threshold.is_none() => return Ok(None),
            ("none", ..) => return Err(SettingError::OptionsForNone),
            ("jev", model, None) => Provider::Jev {
                model: nonempty(model, "--model")?.unwrap_or(DEFAULT_MODEL).into(),
            },
            ("jev", _, Some(_)) => return Err(SettingError::Misplaced("--command", "command")),
            ("command", None, command) => match nonempty(command, "--command")? {
                Some(command) => Provider::Command {
                    command: command.into(),
                },
                None => return Err(SettingError::NoCommand),
            },
            ("command", Some(_), _) => return Err(SettingError::Misplaced("--model", "jev")),
            (other, ..) => return Err(SettingError::UnknownProvider(other.into())),
        };
        let threshold = threshold.unwrap_or(DEFAULT_THRESHOLD);
        if !(threshold > 0.0 && threshold <= 1.0) {
            return Err(SettingError::Threshold(threshold));
        }
        Ok(Some(Self {
            provider,
            threshold,
        }))
    }
}

fn nonempty<'a>(
    value: Option<&'a str>,
    flag: &'static str,
) -> Result<Option<&'a str>, SettingError> {
    match value.map(str::trim) {
        Some("") => Err(SettingError::Empty(flag)),
        value => Ok(value),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingError {
    #[error("no judge {0:?}; judges: none, jev, command")]
    UnknownProvider(String),
    #[error("the command judge needs --command")]
    NoCommand,
    #[error("{0} is only for the {1} judge")]
    Misplaced(&'static str, &'static str),
    #[error("none takes no options")]
    OptionsForNone,
    #[error("{0} can't be empty")]
    Empty(&'static str),
    #[error("the threshold must be above 0 and at most 1, got {0}")]
    Threshold(f64),
    #[error("no project named {0}")]
    NoProject(ProjectName),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Gives `project` the judge `setting`, or none.
pub fn set<S: ProjectStore + JudgeStore>(
    store: &mut S,
    project: &ProjectName,
    setting: Option<&JudgeSetting>,
) -> Result<(), SettingError> {
    if store.project(project)?.is_none() {
        return Err(SettingError::NoProject(project.clone()));
    }
    store.set_judge_setting(project, setting)?;
    Ok(())
}

#[cfg(test)]
#[path = "setting_tests.rs"]
mod tests;
