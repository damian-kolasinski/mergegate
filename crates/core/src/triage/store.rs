use super::{HeldEdit, Proposal, ProposalState, RuleId, Rules, Triage};
use crate::item::ItemId;
use crate::project::ProjectName;
use crate::store::StoreError;

/// The rules files are triaged by, the changes to them waiting for the
/// operator, and the triage stored with each file.
pub trait RulesStore {
    /// The rules last applied to the project; empty before any.
    fn applied_rules(&self, project: &ProjectName) -> Result<Rules, StoreError>;

    /// The file's text when its rules were last applied, which rejecting an
    /// edit puts back; none while no file was ever applied.
    fn applied_text(&self, project: &ProjectName) -> Result<Option<String>, StoreError>;

    fn set_applied_rules(
        &mut self,
        project: &ProjectName,
        rules: &Rules,
        text: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Replaces the triage of `revision`'s files, by path.
    fn set_triage(
        &mut self,
        id: &ItemId,
        revision: u32,
        triage: &[(String, Triage)],
    ) -> Result<(), StoreError>;

    /// One past every ID the project's applied rules, proposals and held
    /// edits ever used, so no rule takes a retired one; none once a
    /// hand-added rule used the largest.
    fn next_rule_id(&self, project: &ProjectName) -> Result<Option<RuleId>, StoreError>;

    fn insert_proposal(
        &mut self,
        project: &ProjectName,
        proposal: &Proposal,
    ) -> Result<(), StoreError>;

    fn proposal(&self, project: &ProjectName, id: RuleId) -> Result<Option<Proposal>, StoreError>;

    /// Those still waiting, oldest first.
    fn pending_proposals(&self, project: &ProjectName) -> Result<Vec<Proposal>, StoreError>;

    fn set_proposal_state(
        &mut self,
        project: &ProjectName,
        id: RuleId,
        state: ProposalState,
    ) -> Result<(), StoreError>;

    /// The hand edit waiting for the operator, at most one per project.
    fn held_edit(&self, project: &ProjectName) -> Result<Option<HeldEdit>, StoreError>;

    fn set_held_edit(
        &mut self,
        project: &ProjectName,
        edit: Option<&HeldEdit>,
    ) -> Result<(), StoreError>;
}
