//! A hand edit to the rules file that loosens the applied rules, held for
//! the operator's approval in the UI.

use super::sync::retriage;
use super::{ChangeError, Decision, Preview, Rules, RulesFile, RulesStore};
use crate::diff::DiffStore;
use crate::item::{ItemId, ItemStore};
use crate::project::ProjectName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldEdit {
    /// The file's rules when it was held.
    pub rules: Rules,
    /// The file's text then, the applied text once approved.
    pub text: String,
    pub preview: Preview,
}

/// Approving applies the rules the operator saw previewed, even if the file
/// moved on since; the next sync holds or applies what it holds now.
/// Rejecting writes the applied text back to the file.
pub fn decide_edit<S: RulesStore + ItemStore + DiffStore>(
    store: &mut S,
    file: &impl RulesFile,
    project: &ProjectName,
    decision: Decision,
) -> Result<Vec<ItemId>, ChangeError> {
    let edit = store.held_edit(project)?.ok_or(ChangeError::NoEdit)?;
    let moved = match decision {
        Decision::Approve => {
            let moved = retriage(store, project, &edit.rules)?;
            store.set_applied_rules(project, &edit.rules, Some(&edit.text))?;
            moved
        }
        Decision::Reject => {
            let text = store.applied_text(project)?.unwrap_or_default();
            file.write(project, &text)?;
            Vec::new()
        }
    };
    store.set_held_edit(project, None)?;
    Ok(moved)
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
