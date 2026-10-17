//! The language packs a new project asks for: the ones its files use that
//! aren't installed, installed by default.

use std::path::Path;

use super::ProjectError;
use crate::decision::{Decision, DecisionOption};
use crate::git::{Git, GitError};
use crate::highlight::{Detected, Pack, detect};

/// What `mg project add` knows about language packs.
#[derive(Debug, Clone, Default)]
pub struct Languages {
    /// The packs mg can install. Empty skips detection.
    pub known: Vec<Pack>,
    /// The names of the installed packs.
    pub installed: Vec<String>,
    /// The packs the operator chose, empty for none; `None` asks when the
    /// repo uses any that aren't installed.
    pub chosen: Option<Vec<String>>,
}

pub(super) enum Chosen {
    Install(Vec<String>),
    Decide(Decision),
}

/// The value of `--languages` that installs none.
pub const NONE: &str = "none";

pub(super) fn choose(
    git: &impl Git,
    root: &Path,
    target: &str,
    languages: &Languages,
) -> Result<Chosen, ProjectError> {
    if let Some(chosen) = &languages.chosen {
        if let Some(unknown) = chosen
            .iter()
            .find(|name| !languages.known.iter().any(|p| &p.name == *name))
        {
            return Err(ProjectError::InvalidValue {
                key: "languages",
                value: unknown.clone(),
                expected: "language pack names from `mg lang list --available`, or none",
            });
        }
        return Ok(Chosen::Install(chosen.clone()));
    }
    if languages.known.is_empty() {
        return Ok(Chosen::Install(Vec::new()));
    }
    let missing = missing(git, root, target, &languages.known, &languages.installed)?;
    if missing.is_empty() {
        return Ok(Chosen::Install(Vec::new()));
    }
    let names: Vec<&str> = missing.iter().map(|d| d.pack.as_str()).collect();
    let uses = missing
        .iter()
        .map(|d| {
            let s = if d.files == 1 { "" } else { "s" };
            format!("{} ({} file{s})", d.pack, d.files)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let all = names.join(",");
    Ok(Chosen::Decide(Decision {
        question: format!(
            "{} uses {uses}, which have no language pack installed. \
             Install them so reviews are syntax highlighted?",
            root.display()
        ),
        flag: "--languages".into(),
        options: vec![
            DecisionOption {
                value: all.clone(),
                description: Some("download and install these packs on the hub".into()),
            },
            DecisionOption {
                value: NONE.into(),
                description: Some(
                    "show these files as plain text; mg lang add installs packs later".into(),
                ),
            },
        ],
        default: Some(all),
    }))
}

/// The packs of `known` that the files on `target` in the repo at `root`
/// use and that aren't `installed`, most files first.
pub fn missing(
    git: &impl Git,
    root: &Path,
    target: &str,
    known: &[Pack],
    installed: &[String],
) -> Result<Vec<Detected>, GitError> {
    let files = git.files(root, target)?;
    Ok(detect(files.iter().map(String::as_str), known)
        .into_iter()
        .filter(|d| !installed.contains(&d.pack))
        .collect())
}

#[cfg(test)]
#[path = "languages_tests.rs"]
mod tests;
