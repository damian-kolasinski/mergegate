//! In-memory ports for the domain's unit tests. The adapters are tested
//! against real git and SQLite in their own crates.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::git::{Git, GitError, Repo};
use crate::project::{Project, ProjectName, ProjectStore};
use crate::store::StoreError;

pub(crate) struct FakeGit {
    repos: Vec<Repo>,
    version: Option<String>,
    fail_sync: bool,
    /// Targets in each project's staging repo.
    staging: RefCell<BTreeMap<String, BTreeSet<String>>>,
    synced: RefCell<Vec<(String, PathBuf, String)>>,
}

impl Default for FakeGit {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            version: Some("git version 2.50.1".into()),
            fail_sync: false,
            staging: RefCell::default(),
            synced: RefCell::default(),
        }
    }
}

impl FakeGit {
    /// Adds the repo at `root`, replacing one already there.
    pub fn repo(self, root: &str, branches: &[&str], head: Option<&str>) -> Self {
        let mut git = self.without_repo(root);
        let mut branches: Vec<String> = branches.iter().map(|b| (*b).into()).collect();
        branches.sort();
        git.repos.push(Repo {
            root: root.into(),
            branches,
            head: head.map(Into::into),
        });
        git
    }

    pub fn without_repo(mut self, root: &str) -> Self {
        self.repos.retain(|r| r.root != Path::new(root));
        self
    }

    pub fn without_staging(self, project: &str) -> Self {
        self.staging.borrow_mut().remove(project);
        self
    }

    pub fn failing_sync(mut self) -> Self {
        self.fail_sync = true;
        self
    }

    pub fn version(mut self, output: Option<&str>) -> Self {
        self.version = output.map(Into::into);
        self
    }

    /// Every `sync_staging` call: project, upstream, target.
    pub fn synced(&self) -> Vec<(String, PathBuf, String)> {
        self.synced.borrow().clone()
    }
}

impl Git for FakeGit {
    fn inspect(&self, path: &Path) -> Result<Repo, GitError> {
        self.repos
            .iter()
            .filter(|r| path.starts_with(&r.root))
            .max_by_key(|r| r.root.components().count())
            .cloned()
            .ok_or_else(|| GitError::NotARepo(path.into()))
    }

    fn sync_staging(
        &self,
        project: &ProjectName,
        upstream: &Path,
        target: &str,
    ) -> Result<(), GitError> {
        if self.fail_sync {
            return Err(GitError::Failed("fetch failed".into()));
        }
        self.synced
            .borrow_mut()
            .push((project.to_string(), upstream.into(), target.into()));
        self.staging
            .borrow_mut()
            .entry(project.to_string())
            .or_default()
            .insert(target.into());
        Ok(())
    }

    fn staging_has(&self, project: &ProjectName, target: &str) -> Result<bool, GitError> {
        Ok(self
            .staging
            .borrow()
            .get(project.as_str())
            .is_some_and(|targets| targets.contains(target)))
    }

    fn version(&self) -> Result<String, GitError> {
        self.version
            .clone()
            .ok_or_else(|| GitError::Failed("git: command not found".into()))
    }
}

#[derive(Default)]
pub(crate) struct FakeStore {
    projects: BTreeMap<ProjectName, Project>,
}

impl ProjectStore for FakeStore {
    fn projects(&self) -> Result<Vec<Project>, StoreError> {
        Ok(self.projects.values().cloned().collect())
    }

    fn project(&self, name: &ProjectName) -> Result<Option<Project>, StoreError> {
        Ok(self.projects.get(name).cloned())
    }

    fn project_with_upstream(&self, upstream: &Path) -> Result<Option<Project>, StoreError> {
        Ok(self
            .projects
            .values()
            .find(|p| p.upstream == upstream)
            .cloned())
    }

    fn insert_project(&mut self, project: &Project) -> Result<(), StoreError> {
        let previous = self.projects.insert(project.name.clone(), project.clone());
        assert!(previous.is_none(), "inserted {} twice", project.name);
        Ok(())
    }

    fn update_project(&mut self, project: &Project) -> Result<(), StoreError> {
        let previous = self.projects.insert(project.name.clone(), project.clone());
        assert!(previous.is_some(), "updated missing {}", project.name);
        Ok(())
    }
}
