//! Which branch a new project's items merge into.

use api::decision::{DecisionOption, NeedsDecision};

use super::ProjectError;
use crate::git::Repo;

/// Branch names that, alone in a repo, are the target without asking.
const CONVENTIONAL: [&str; 3] = ["main", "master", "trunk"];

/// How many branches a decision offers when none is conventional.
const MAX_OPTIONS: usize = 10;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Target {
    Chosen(String),
    Decide(NeedsDecision),
}

pub(super) fn choose(repo: &Repo, requested: Option<&str>) -> Result<Target, ProjectError> {
    if let Some(branch) = requested {
        if !repo.has_branch(branch) {
            return Err(ProjectError::UnknownBranch {
                branch: branch.into(),
                upstream: repo.root.clone(),
            });
        }
        return Ok(Target::Chosen(branch.into()));
    }
    if repo.branches.is_empty() {
        return Err(ProjectError::NoCommits(repo.root.clone()));
    }
    let conventional: Vec<&str> = CONVENTIONAL
        .into_iter()
        .filter(|b| repo.has_branch(b))
        .collect();
    if let [only] = conventional.as_slice() {
        return Ok(Target::Chosen((*only).into()));
    }
    if let [only] = repo.branches.as_slice() {
        return Ok(Target::Chosen(only.clone()));
    }

    let head = repo.head.as_deref().filter(|h| repo.has_branch(h));
    let mut candidates: Vec<&str> = if conventional.is_empty() {
        repo.branches.iter().map(String::as_str).collect()
    } else {
        conventional
    };
    if candidates.len() > MAX_OPTIONS {
        candidates.retain(|b| Some(*b) != head);
        candidates.truncate(MAX_OPTIONS - usize::from(head.is_some()));
        candidates.extend(head);
        candidates.sort_unstable();
    }
    let default = head
        .filter(|h| candidates.contains(h))
        .unwrap_or(candidates[0]);
    Ok(Target::Decide(NeedsDecision {
        question: format!(
            "Which branch of {} should the project's items merge into?",
            repo.root.display()
        ),
        flag: "--target".into(),
        options: candidates
            .into_iter()
            .map(|b| DecisionOption {
                value: b.into(),
                description: (Some(b) == head).then(|| "checked out".into()),
            })
            .collect(),
        default: Some(default.into()),
    }))
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
