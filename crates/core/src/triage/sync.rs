//! Applying the operator's rules file, and re-triaging the open items when
//! its rules change.

use super::triage_files;
use super::{HeldEdit, Rules, RulesFile, RulesFileError, RulesStore, loosens, preview};
use crate::diff::DiffStore;
use crate::item::{ItemId, ItemStore};
use crate::project::ProjectName;
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Synced {
    /// Nothing the operator sees changed.
    Unchanged,
    /// The file's rules now apply; these open items' files moved between
    /// tiers or changed rule.
    Applied(Vec<ItemId>),
    /// The file loosens the applied rules, so its edit waits for the
    /// operator, with a new preview.
    Held,
    /// The file can't be used, so the applied rules stay.
    Invalid(String),
}

/// Reads the file and applies its rules if they differ from the applied
/// ones and hide nothing more, else holds them. No file means no rules.
pub fn sync<S: RulesStore + ItemStore + DiffStore>(
    store: &mut S,
    file: &impl RulesFile,
    project: &ProjectName,
) -> Result<Synced, StoreError> {
    let (rules, text) = match file.read(project) {
        Ok(Some(file)) => (file.rules, Some(file.text)),
        Ok(None) => (Vec::new(), None),
        Err(RulesFileError::Invalid(e) | RulesFileError::Io(e)) => {
            return Ok(Synced::Invalid(e));
        }
    };
    let rules = match Rules::new(rules) {
        Ok(rules) => rules,
        Err(e) => return Ok(Synced::Invalid(e.to_string())),
    };
    let applied = store.applied_rules(project)?;
    let held = store.held_edit(project)?;
    if let Some(taken) = taken_id(store, project, &rules, &applied, held.as_ref())? {
        return Ok(Synced::Invalid(taken));
    }
    if applied == rules {
        // Comments and layout, which rejecting a later edit puts back too.
        if store.applied_text(project)? != text {
            store.set_applied_rules(project, &rules, text.as_deref())?;
        }
        if held.is_none() {
            return Ok(Synced::Unchanged);
        }
        // The operator undid the edit by hand.
        store.set_held_edit(project, None)?;
        return Ok(Synced::Applied(Vec::new()));
    }
    if !loosens(&applied, &rules) {
        // Recorded last, so a re-triage that fails partway runs again on
        // the next sync.
        let moved = retriage(store, project, &rules)?;
        store.set_applied_rules(project, &rules, text.as_deref())?;
        if held.is_some() {
            store.set_held_edit(project, None)?;
        }
        return Ok(Synced::Applied(moved));
    }
    if held.is_some_and(|held| held.rules == rules) {
        return Ok(Synced::Unchanged);
    }
    let preview = preview(store, project, &applied, &rules)?;
    let edit = HeldEdit {
        rules,
        text: text.unwrap_or_default(),
        preview,
    };
    store.set_held_edit(project, Some(&edit))?;
    Ok(Synced::Held)
}

/// A rule of the file's that takes an ID the hub gave out before, to a
/// proposal or a rule since removed, as a hand-added one can. A reused ID
/// would make a waiting proposal clash on approval and stored triage name
/// the wrong rule.
fn taken_id<S: RulesStore>(
    store: &S,
    project: &ProjectName,
    rules: &Rules,
    applied: &Rules,
    held: Option<&HeldEdit>,
) -> Result<Option<String>, StoreError> {
    let next = store.next_rule_id(project)?;
    let known = |id| {
        applied.rules().any(|r| r.id == id)
            || held.is_some_and(|held| held.rules.rules().any(|r| r.id == id))
    };
    Ok(rules
        .rules()
        .find(|rule| rule.id < next && !known(rule.id))
        .map(|rule| format!("{} is taken by an earlier rule; use {next}", rule.id)))
}

/// Every revision of an open item, not just the newest: an older one is
/// still shown when its comments are answered.
pub(super) fn retriage<S: RulesStore + ItemStore + DiffStore>(
    store: &mut S,
    project: &ProjectName,
    rules: &Rules,
) -> Result<Vec<ItemId>, StoreError> {
    let mut changed = Vec::new();
    for item in store.open_items(Some(project))? {
        let mut moved = false;
        for revision in 1..=item.revision.number {
            let before = store.revision_files(&item.id, revision)?;
            let mut files = before.clone();
            triage_files(store, rules, &mut files)?;
            if files == before {
                continue;
            }
            moved = true;
            let triage: Vec<_> = files.into_iter().map(|f| (f.file.path, f.triage)).collect();
            store.set_triage(&item.id, revision, &triage)?;
        }
        if moved {
            changed.push(item.id);
        }
    }
    Ok(changed)
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
