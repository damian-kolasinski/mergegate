use serde::{Deserialize, Serialize};

/// `GET /api/projects/{name}/judge`, and the answer to [`SetJudge`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Judge {
    pub project: String,
    /// `none`, `jev` or `command`.
    pub provider: String,
    /// Jev's model.
    pub model: Option<String>,
    /// The command, run by `sh -c` in the project's repo.
    pub command: Option<String>,
    /// The confidence from which the judge pulls a trusted file back into
    /// review; null with no judge.
    pub threshold: Option<f64>,
    /// What leads the order of the files to review: `guide`, where the
    /// judge raises only the files it scores 80 or more, or `judge`, whose
    /// scores order them all; null with no judge.
    pub order: Option<String>,
    /// Why the judge can't run as set, as Jev without its key.
    pub problem: Option<String>,
    /// Open items whose newest revision the judge hasn't scored yet,
    /// failures included.
    pub waiting: u32,
    /// Revisions the judge failed on. They aren't tried again until the
    /// judge is set again or the hub restarts.
    pub failures: Vec<Failure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    /// As in `app#42`.
    pub item: String,
    /// As in [`crate::item::Item`].
    pub name: String,
    pub revision: u32,
    pub error: String,
}

/// `PUT /api/projects/{name}/judge`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetJudge {
    /// `none`, `jev` or `command`.
    pub provider: String,
    pub model: Option<String>,
    pub command: Option<String>,
    pub threshold: Option<f64>,
    /// `guide` or `judge` [default: guide].
    pub order: Option<String>,
}
