//! Rules an agent proposes when the operator asks: a `review` rule applies at
//! once, a `trusted` one waits for the operator's approval in the UI.

use super::file::{Place, RulesFile, RulesFileError};
use super::sync::retriage;
use super::{Kind, Preview, Rule, RuleError, RuleId, Rules, RulesStore, Synced, Tier};
use super::{preview, sync};
use crate::diff::DiffStore;
use crate::item::{ItemId, ItemStore};
use crate::project::ProjectName;
use crate::store::StoreError;

/// A rule as proposed, before the hub numbers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRule {
    pub tier: Tier,
    pub path: Option<String>,
    pub kind: Option<Kind>,
    pub max_lines: Option<u64>,
    pub diff_matches: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub rule: Rule,
    pub state: ProposalState,
    /// None for a `review` rule, which hides nothing.
    pub preview: Option<Preview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalState {
    Pending,
    Applied,
    Rejected,
}

impl ProposalState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(state: &str) -> Option<Self> {
        [Self::Pending, Self::Applied, Self::Rejected]
            .into_iter()
            .find(|s| s.as_str() == state)
    }
}

/// The operator's answer to a change waiting for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposed {
    pub proposal: Proposal,
    /// Open items whose files moved, when the rule applied.
    pub moved: Vec<ItemId>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChangeError {
    #[error("no rule {0} was proposed")]
    NotFound(RuleId),
    #[error("{0} is already {state}", state = .1.as_str())]
    NotPending(RuleId, ProposalState),
    #[error("no edit to the rules file waits for the operator")]
    NoEdit,
    #[error(transparent)]
    Rule(#[from] RuleError),
    /// The hub only writes to a file it can parse, so the operator fixes it
    /// first.
    #[error("{0}")]
    InvalidFile(String),
    #[error("{0}")]
    File(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<RulesFileError> for ChangeError {
    fn from(e: RulesFileError) -> Self {
        match e {
            RulesFileError::Invalid(e) => Self::InvalidFile(e),
            RulesFileError::Io(e) => Self::File(e),
        }
    }
}

/// Numbers `rule` and applies it, or stores it to wait when it's `trusted`,
/// with what it would have hidden.
pub fn propose<S: RulesStore + ItemStore + DiffStore>(
    store: &mut S,
    file: &impl RulesFile,
    project: &ProjectName,
    rule: NewRule,
) -> Result<Proposed, ChangeError> {
    // So the ID passes those hand-added since the last sync, and the
    // preview compares with the rules the file holds.
    let mut moved = synced(store, file, project)?;
    let rule = Rule {
        id: store.next_rule_id(project)?,
        tier: rule.tier,
        path: rule.path,
        kind: rule.kind,
        max_lines: rule.max_lines,
        diff_matches: rule.diff_matches,
        reason: rule.reason,
    };
    Rules::new(vec![rule.clone()])?;
    let proposal = match rule.tier {
        Tier::Review => {
            moved.extend(add_rule(store, file, project, &rule)?);
            Proposal {
                rule,
                state: ProposalState::Applied,
                preview: None,
            }
        }
        Tier::Trusted => {
            let applied = store.applied_rules(project)?;
            let mut next: Vec<_> = applied.rules().cloned().collect();
            Place::of(rule.tier).insert(&mut next, rule.clone());
            let preview = preview(store, project, &applied, &Rules::new(next)?)?;
            Proposal {
                rule,
                state: ProposalState::Pending,
                preview: Some(preview),
            }
        }
    };
    store.insert_proposal(project, &proposal)?;
    Ok(Proposed { proposal, moved })
}

/// Approving adds the rule last, after the rules applied since it was
/// proposed; rejecting leaves the rules as they are.
pub fn decide_proposal<S: RulesStore + ItemStore + DiffStore>(
    store: &mut S,
    file: &impl RulesFile,
    project: &ProjectName,
    id: RuleId,
    decision: Decision,
) -> Result<Vec<ItemId>, ChangeError> {
    let proposal = store
        .proposal(project, id)?
        .ok_or(ChangeError::NotFound(id))?;
    if proposal.state != ProposalState::Pending {
        return Err(ChangeError::NotPending(id, proposal.state));
    }
    let (moved, state) = match decision {
        Decision::Approve => (
            add_rule(store, file, project, &proposal.rule)?,
            ProposalState::Applied,
        ),
        Decision::Reject => (Vec::new(), ProposalState::Rejected),
    };
    store.set_proposal_state(project, id, state)?;
    Ok(moved)
}

/// Writes `rule` into the file where [`Place::of`] puts it and applies it,
/// returning the open items whose files moved.
fn add_rule<S: RulesStore + ItemStore + DiffStore>(
    store: &mut S,
    file: &impl RulesFile,
    project: &ProjectName,
    rule: &Rule,
) -> Result<Vec<ItemId>, ChangeError> {
    // So the file's applied text is current, and a held edit is the file's
    // own rules.
    let mut moved = synced(store, file, project)?;
    let place = Place::of(rule.tier);
    let text = file.read(project)?.map(|f| f.text).unwrap_or_default();
    let written = file.insert(&text, rule, place)?;
    let held = store.held_edit(project)?;
    // A held edit's rules aren't applied, so the text rejecting it puts
    // back is the applied one with the rule, not the file.
    let applied_text = match &held {
        Some(_) => {
            let applied = store.applied_text(project)?.unwrap_or_default();
            file.insert(&applied, rule, place)?
        }
        None => written.clone(),
    };
    let mut rules: Vec<_> = store.applied_rules(project)?.rules().cloned().collect();
    place.insert(&mut rules, rule.clone());
    let rules = Rules::new(rules)?;
    file.write(project, &written)?;
    moved.extend(retriage(store, project, &rules)?);
    store.set_applied_rules(project, &rules, Some(&applied_text))?;
    if held.is_some() {
        // Held again with the rule in, and a preview against the new rules.
        store.set_held_edit(project, None)?;
        moved.extend(synced(store, file, project)?);
    }
    moved.sort_by_key(ToString::to_string);
    moved.dedup();
    Ok(moved)
}

fn synced<S: RulesStore + ItemStore + DiffStore>(
    store: &mut S,
    file: &impl RulesFile,
    project: &ProjectName,
) -> Result<Vec<ItemId>, ChangeError> {
    match sync(store, file, project)? {
        Synced::Applied(moved) => Ok(moved),
        Synced::Unchanged | Synced::Held => Ok(Vec::new()),
        Synced::Invalid(e) => Err(ChangeError::InvalidFile(format!(
            "{} can't be used: {e}",
            file.location(project)
        ))),
    }
}

#[cfg(test)]
#[path = "propose_tests.rs"]
mod tests;
