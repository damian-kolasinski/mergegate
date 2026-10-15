//! File triage: which group each changed file of a revision lands in
//! (design §7). Only a rule the operator approved can hide a file.

mod classify;
mod edit;
mod file;
mod git;
mod guide;
mod heuristic;
mod loosens;
mod order;
mod preview;
mod propose;
mod rules;
mod store;
mod sync;

pub use classify::{GITATTRIBUTES, classify, triage_files};
pub use edit::{HeldEdit, decide_edit};
pub use file::{Place, RulesFile, RulesFileError, RulesText};
pub use git::TriageGit;
pub use guide::{Guide, GuideFile};
pub use heuristic::{Hint, hint};
pub use loosens::loosens;
pub use order::{OrderBy, Placed, RAISE_AT, Rank, order};
pub use preview::{Hidden, MERGED, Preview, preview};
pub use propose::{
    ChangeError, Decision, NewRule, Proposal, ProposalState, Proposed, decide_proposal, propose,
};
pub use rules::{Kind, Rule, RuleError, RuleId, Rules};
pub use store::RulesStore;
pub use sync::{Synced, sync};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tier {
    /// Shown, ordered by importance.
    #[default]
    Review,
    /// Folded away and counted as viewed.
    Trusted,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Trusted => "trusted",
        }
    }

    pub fn parse(tier: &str) -> Option<Self> {
        [Self::Review, Self::Trusted]
            .into_iter()
            .find(|t| t.as_str() == tier)
    }
}

/// A file's group and what put it there.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Triage {
    pub tier: Tier,
    /// None when nothing matched and the file is reviewed by default.
    pub by: Option<TriagedBy>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriagedBy {
    /// Always `review`, before any rule.
    Guard(Guard),
    /// `why` is the rule's matchers and reason as they were when it matched,
    /// so a file still explains itself after the rule changes.
    Rule { id: RuleId, why: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    /// The file can mark others `linguist-generated`, which rules match on.
    Gitattributes,
}

impl Guard {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gitattributes => "gitattributes",
        }
    }

    pub fn parse(guard: &str) -> Option<Self> {
        (guard == "gitattributes").then_some(Self::Gitattributes)
    }

    pub fn why(self) -> &'static str {
        match self {
            Self::Gitattributes => "it can mark files generated, which rules match on",
        }
    }
}
