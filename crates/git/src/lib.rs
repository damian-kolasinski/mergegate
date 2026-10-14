//! The git adapter. Reads go through gix, in process. Writes run the `git`
//! binary, which the hub needs anyway for merging and which handles every
//! transport and repo format the operator's repos may use.

mod command;
mod diff;
mod http;
mod inspect;
mod items;
mod merge;
mod publish;
mod remote;
mod staging;
mod trial;

use std::path::{Path, PathBuf};

use mergegate_core::git::{Git, GitError, Repo, Upstream};
use mergegate_core::project::ProjectName;

pub use http::HttpRequest;

pub struct HubGit {
    projects: PathBuf,
    home: PathBuf,
}

impl HubGit {
    /// Staging repos go under `projects`, one folder per project. `home` is
    /// what the `git` binary sees as `HOME`, so no settings of the user
    /// running the hub apply; it doesn't need to exist.
    pub fn new(projects: PathBuf, home: PathBuf) -> Self {
        Self { projects, home }
    }

    fn staging(&self, project: &ProjectName) -> PathBuf {
        self.projects.join(project.as_str()).join("staging.git")
    }
}

impl Git for HubGit {
    fn inspect(&self, path: &Path) -> Result<Repo, GitError> {
        inspect::inspect(path)
    }

    fn sync_staging(
        &self,
        project: &ProjectName,
        upstream: &Upstream,
        target: &str,
    ) -> Result<(), GitError> {
        staging::sync(&self.home, &self.staging(project), upstream, target)
    }

    fn staging_has(&self, project: &ProjectName, target: &str) -> Result<bool, GitError> {
        staging::has(&self.staging(project), target)
    }

    fn version(&self) -> Result<String, GitError> {
        command::run(&self.home, "running git --version", ["--version"])
    }
}
