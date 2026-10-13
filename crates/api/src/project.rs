use serde::{Deserialize, Serialize};

/// Where a project's approved work lands.
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
}

/// `PATCH /api/projects/{name}`: changes one setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetProject {
    pub key: String,
    pub value: String,
}

/// `GET /api/project-at?path=`: the project whose repo contains `path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAt {
    pub path: String,
}
