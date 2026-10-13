//! In-memory ports for the domain's unit tests. The adapters are tested
//! against real git and SQLite in their own crates.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::git::{Git, GitError, Repo};
use crate::item::{
    DiffStat, FileStat, Item, ItemGit, ItemId, ItemState, ItemStore, NewItem, NewRevision, Revision,
};
use crate::project::{Project, ProjectName, ProjectStore};
use crate::store::StoreError;

/// A commit ID for tests: `n` as 40 hex digits.
pub(crate) fn commit(n: u8) -> String {
    format!("{n:040x}")
}

struct Commit {
    parent: Option<String>,
    subject: String,
    files: Vec<String>,
}

pub(crate) struct FakeGit {
    repos: Vec<Repo>,
    version: Option<String>,
    fail_sync: bool,
    /// Commits of every repo, by ID. Each has at most one parent.
    commits: BTreeMap<String, Commit>,
    /// Branch heads in the upstream repos, by root and branch.
    upstream: BTreeMap<(PathBuf, String), String>,
    /// Branches in each project's staging repo, with their heads.
    staging: RefCell<BTreeMap<String, BTreeMap<String, String>>>,
    synced: RefCell<Vec<(String, PathBuf, String)>>,
    fetched: RefCell<Vec<(String, String)>>,
    pins: RefCell<BTreeMap<String, String>>,
}

impl Default for FakeGit {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            version: Some("git version 2.50.1".into()),
            fail_sync: false,
            commits: BTreeMap::new(),
            upstream: BTreeMap::new(),
            staging: RefCell::default(),
            synced: RefCell::default(),
            fetched: RefCell::default(),
            pins: RefCell::default(),
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

    /// Adds commit `id` on top of `parent`, changing `files` by one line each.
    pub fn commit(mut self, id: &str, parent: Option<&str>, subject: &str, files: &[&str]) -> Self {
        let commit = Commit {
            parent: parent.map(Into::into),
            subject: subject.into(),
            files: files.iter().map(|f| (*f).into()).collect(),
        };
        self.commits.insert(id.into(), commit);
        self
    }

    /// Points `branch` of the upstream repo at `root` to `commit`.
    pub fn upstream(mut self, root: &str, branch: &str, commit: &str) -> Self {
        self.upstream
            .insert((root.into(), branch.into()), commit.into());
        self
    }

    /// What a worker's `git push` does: points `branch` in staging at `commit`.
    pub fn push(&self, project: &str, branch: &str, commit: &str) {
        self.staging
            .borrow_mut()
            .entry(project.into())
            .or_default()
            .insert(branch.into(), commit.into());
    }

    /// Every `sync_staging` call: project, upstream, target.
    pub fn synced(&self) -> Vec<(String, PathBuf, String)> {
        self.synced.borrow().clone()
    }

    /// Every `fetch_target` call: project, target.
    pub fn fetched(&self) -> Vec<(String, String)> {
        self.fetched.borrow().clone()
    }

    /// Pinned refs with their commits.
    pub fn pins(&self) -> BTreeMap<String, String> {
        self.pins.borrow().clone()
    }

    /// `id` and its ancestors, newest first.
    fn history(&self, id: &str) -> Vec<String> {
        let mut history = Vec::new();
        let mut next = Some(id.to_owned());
        while let Some(id) = next {
            next = self.commits.get(&id).and_then(|c| c.parent.clone());
            history.push(id);
        }
        history
    }

    /// The commits after `base` up to `head`, newest first.
    fn range(&self, base: &str, head: &str) -> Vec<String> {
        self.history(head)
            .into_iter()
            .take_while(|id| id != base)
            .collect()
    }

    fn fetch(&self, project: &ProjectName, upstream: &Path, target: &str) -> Result<(), GitError> {
        if self.fail_sync {
            return Err(GitError::Failed("fetch failed".into()));
        }
        let head = self
            .upstream
            .get(&(upstream.into(), target.into()))
            .cloned()
            .unwrap_or_default();
        self.push(project.as_str(), target, &head);
        Ok(())
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
        self.fetch(project, upstream, target)?;
        self.synced
            .borrow_mut()
            .push((project.to_string(), upstream.into(), target.into()));
        Ok(())
    }

    fn staging_has(&self, project: &ProjectName, target: &str) -> Result<bool, GitError> {
        Ok(self
            .staging
            .borrow()
            .get(project.as_str())
            .is_some_and(|branches| branches.contains_key(target)))
    }

    fn version(&self) -> Result<String, GitError> {
        self.version
            .clone()
            .ok_or_else(|| GitError::Failed("git: command not found".into()))
    }
}

impl ItemGit for FakeGit {
    fn fetch_target(
        &self,
        project: &ProjectName,
        upstream: &Path,
        target: &str,
    ) -> Result<(), GitError> {
        self.fetch(project, upstream, target)?;
        self.fetched
            .borrow_mut()
            .push((project.to_string(), target.into()));
        Ok(())
    }

    fn branch_head(&self, project: &ProjectName, branch: &str) -> Result<Option<String>, GitError> {
        Ok(self
            .staging
            .borrow()
            .get(project.as_str())
            .and_then(|branches| branches.get(branch).cloned()))
    }

    fn merge_base(
        &self,
        project: &ProjectName,
        head: &str,
        target: &str,
    ) -> Result<Option<String>, GitError> {
        let target = self
            .branch_head(project, target)?
            .ok_or_else(|| GitError::Failed(format!("no branch {target}")))?;
        let ours: BTreeSet<String> = self.history(head).into_iter().collect();
        Ok(self
            .history(&target)
            .into_iter()
            .find(|id| ours.contains(id)))
    }

    fn changed_files(
        &self,
        _project: &ProjectName,
        base: &str,
        head: &str,
    ) -> Result<Vec<FileStat>, GitError> {
        let mut lines: BTreeMap<String, u64> = BTreeMap::new();
        for id in self.range(base, head) {
            for file in &self.commits[&id].files {
                *lines.entry(file.clone()).or_default() += 1;
            }
        }
        Ok(lines
            .into_iter()
            .map(|(path, added)| FileStat {
                path,
                old_path: None,
                additions: Some(added),
                deletions: Some(0),
            })
            .collect())
    }

    fn first_subject(
        &self,
        _project: &ProjectName,
        base: &str,
        head: &str,
    ) -> Result<Option<String>, GitError> {
        Ok(self
            .range(base, head)
            .last()
            .map(|id| self.commits[id].subject.clone()))
    }

    fn pin(&self, _project: &ProjectName, reference: &str, commit: &str) -> Result<(), GitError> {
        self.pins
            .borrow_mut()
            .insert(reference.into(), commit.into());
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct FakeStore {
    projects: BTreeMap<ProjectName, Project>,
    /// Oldest first.
    items: Vec<Item>,
    /// Ticks once per stamped time.
    clock: u32,
}

impl FakeStore {
    /// Every item, open or not, oldest first.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Moves an item to `state`, as reviews and merges will.
    pub fn set_state(&mut self, id: &ItemId, state: ItemState) {
        self.items.iter_mut().find(|i| i.id == *id).unwrap().state = state;
    }

    fn now(&mut self) -> String {
        self.clock += 1;
        format!("2026-01-01T00:00:{:02}Z", self.clock)
    }

    fn revision(&mut self, number: u32, revision: NewRevision) -> Revision {
        Revision {
            number,
            stat: DiffStat::of(&revision.files),
            head: revision.head,
            base: revision.base,
            submitted_at: self.now(),
        }
    }
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

impl ItemStore for FakeStore {
    fn open_items(&self, project: Option<&ProjectName>) -> Result<Vec<Item>, StoreError> {
        Ok(self
            .items
            .iter()
            .filter(|i| i.state.is_open())
            .filter(|i| project.is_none_or(|p| i.id.project() == p))
            .cloned()
            .collect())
    }

    fn open_item(&self, project: &ProjectName, branch: &str) -> Result<Option<Item>, StoreError> {
        Ok(self
            .open_items(Some(project))?
            .into_iter()
            .find(|i| i.branch == branch))
    }

    fn insert_item(&mut self, item: NewItem) -> Result<Item, StoreError> {
        assert!(
            self.open_item(&item.project, &item.branch)?.is_none(),
            "{} already has an open item",
            item.branch
        );
        let number = self
            .items
            .iter()
            .filter(|i| *i.id.project() == item.project)
            .count() as u64
            + 1;
        let revision = self.revision(1, item.revision);
        let item = Item {
            id: ItemId::new(item.project, number),
            branch: item.branch,
            title: item.title,
            state: item.state,
            backend: item.backend,
            target: item.target,
            created_at: revision.submitted_at.clone(),
            revision,
        };
        self.items.push(item.clone());
        Ok(item)
    }

    fn add_revision(
        &mut self,
        id: &ItemId,
        title: &str,
        state: ItemState,
        revision: NewRevision,
    ) -> Result<Item, StoreError> {
        let index = self.items.iter().position(|i| i.id == *id).unwrap();
        let number = self.items[index].revision.number + 1;
        let revision = self.revision(number, revision);
        let item = &mut self.items[index];
        item.title = title.into();
        item.state = state;
        item.revision = revision;
        Ok(item.clone())
    }
}
