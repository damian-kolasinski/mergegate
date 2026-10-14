//! Applying the operator's rules file, and re-triaging the open items when
//! its rules change.

use super::{Rules, RulesFile, RulesFileError, RulesStore, triage_files};
use crate::diff::DiffStore;
use crate::item::{ItemId, ItemStore};
use crate::project::ProjectName;
use crate::store::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Synced {
    /// The file holds the applied rules.
    Unchanged,
    /// The file's rules now apply; these open items' files moved between
    /// tiers or changed rule.
    Applied(Vec<ItemId>),
    /// The file can't be used, so the applied rules stay.
    Invalid(String),
}

/// Reads the file and applies its rules if they differ from the applied
/// ones. No file means no rules.
pub fn sync<S: RulesStore + ItemStore + DiffStore>(
    store: &mut S,
    file: &impl RulesFile,
    project: &ProjectName,
) -> Result<Synced, StoreError> {
    let rules = match file.read(project) {
        Ok(rules) => rules,
        Err(RulesFileError::Invalid(e) | RulesFileError::Unreadable(e)) => {
            return Ok(Synced::Invalid(e));
        }
    };
    let rules = match Rules::new(rules) {
        Ok(rules) => rules,
        Err(e) => return Ok(Synced::Invalid(e.to_string())),
    };
    if store.applied_rules(project)? == rules {
        return Ok(Synced::Unchanged);
    }
    // Recorded last, so a re-triage that fails partway runs again on the
    // next sync.
    let moved = retriage(store, project, &rules)?;
    store.set_applied_rules(project, &rules)?;
    Ok(Synced::Applied(moved))
}

/// Every revision of an open item, not just the newest: an older one is
/// still shown when its comments are answered.
fn retriage<S: RulesStore + ItemStore + DiffStore>(
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
