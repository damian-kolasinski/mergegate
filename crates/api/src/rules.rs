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
