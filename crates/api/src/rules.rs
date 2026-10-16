use serde::{Deserialize, Serialize};

/// `GET /api/projects/{name}/rules`: the rules the project's files are
/// triaged by, first match wins.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    pub project: String,
    /// The rules file the operator edits.
    pub location: String,
    /// The applied rules, in order.
    pub rules: Vec<Rule>,
    /// Set while the file can't be used; the applied rules stay until it's
    /// fixed.
    pub error: Option<String>,
    /// Proposed `trusted` rules waiting for the operator, oldest first.
    pub pending: Vec<Proposal>,
    /// The hand edit to the file waiting for the operator.
    pub edit: Option<Edit>,
}

/// `POST /api/projects/{name}/rules/proposals`, sent when the operator asks
/// for a rule: a `review` one applies at once, a `trusted` one waits for
/// their approval.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposeRule {
    pub tier: Tier,
    pub path: Option<String>,
    pub kind: Option<Kind>,
    pub max_lines: Option<u64>,
    pub diff_matches: Option<String>,
    pub reason: String,
}

/// A proposed rule, as `POST .../rules/proposals` created it and
/// `GET .../rules/proposals/{id}` reads it.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub rule: Rule,
    pub state: ProposalState,
    /// What a `trusted` rule would have hid; none for a `review` one.
    pub preview: Option<Preview>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalState {
    Pending,
    Applied,
    Rejected,
}

/// A hand edit that would hide more, held until the operator approves it;
/// rejecting writes the applied rules back to the file.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    /// The file's rules, in order.
    pub rules: Vec<Rule>,
    pub preview: Preview,
}

/// What a change would have hidden had it applied, computed when it
/// arrived over the open items and the last merged ones.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    /// The items looked at.
    pub items: u32,
    /// Files the applied rules review and the change would have trusted.
    pub hidden: Vec<HiddenFile>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HiddenFile {
    /// As in `app#42`.
    pub item: String,
    /// As in [`crate::item::Item`].
    pub name: String,
    pub title: String,
    pub path: String,
    /// The rule that would have trusted it, as in `R3`.
    pub rule: String,
}

/// A project's changes to its rules waiting for the operator, in the inbox.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingRules {
    pub project: String,
    /// Proposals, plus one for a held edit.
    pub changes: u32,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// As in `R3`.
    pub id: String,
    pub tier: Tier,
    pub path: Option<String>,
    pub kind: Option<Kind>,
    pub max_lines: Option<u64>,
    pub diff_matches: Option<String>,
    pub reason: String,
    /// The matchers in one line, as in ``"`*.lock`, generated"``.
    pub summary: String,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Shown, ordered by importance.
    Review,
    /// Folded away and counted as viewed.
    Trusted,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Added,
    Deleted,
    Renamed,
    Modified,
    Generated,
}
