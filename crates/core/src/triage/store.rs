use super::{Rules, Triage};
use crate::item::ItemId;
use crate::project::ProjectName;
use crate::store::StoreError;

/// The rules files are triaged by, and the triage stored with each file.
pub trait RulesStore {
    /// The rules last applied to the project; empty before any.
    fn applied_rules(&self, project: &ProjectName) -> Result<Rules, StoreError>;

    fn set_applied_rules(&mut self, project: &ProjectName, rules: &Rules)
    -> Result<(), StoreError>;

    /// Replaces the triage of `revision`'s files, by path.
    fn set_triage(
        &mut self,
        id: &ItemId,
        revision: u32,
        triage: &[(String, Triage)],
    ) -> Result<(), StoreError>;
}
