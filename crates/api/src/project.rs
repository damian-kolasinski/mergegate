use serde::{Deserialize, Serialize};

/// Where a project's approved work lands.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    /// Pushes branches to GitHub, opens PRs and merges through the API.
    Github,
    /// Merges into the target branch of the operator's repo on the hub machine.
    Local,
}

impl Backend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Local => "local",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Prefix of the project's item IDs, as in `app#42`.
    pub name: String,
    pub backend: Backend,
    /// The branch items merge into.
    pub target: String,
    /// The operator's repo the project's work lands in.
    pub upstream: String,
    /// `owner/repo` on GitHub, where the github backend publishes items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
    /// GitHub logins of the bots that review its pull requests: their
    /// comments wait for each other before going to the agent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projects {
    pub projects: Vec<Project>,
}

/// `POST /api/projects`. Answers with the registered [`Project`] or with a
/// `needs_decision`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddProject {
    /// Absolute path inside the repo to register, on the hub's machine.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<Backend>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// `owner/repo` on GitHub; implies the github backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
}

/// `PATCH /api/projects/{name}`: changes one setting. Answers with the
/// [`Project`], or with a `needs_decision` when open items keep the old
/// backend or target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetProject {
    pub key: String,
    pub value: String,
    /// Apply even though open items keep the old setting.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub confirm: bool,
}

/// `GET /api/project-at?path=`: the project whose repo contains `path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAt {
    pub path: String,
}
