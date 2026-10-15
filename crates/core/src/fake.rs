//! In-memory ports for the domain's unit tests. The adapters are tested
//! against real git and SQLite in their own crates.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::agent::{Agent, AgentStore, DueWake};
use crate::attachment::{Attachment, AttachmentStore, Media, NewAttachment};
use crate::diff::{DiffGit, DiffStore, FileDiff};
use crate::external::{Batch, Delivered, External, ExternalStore, Held, NewExternal, Override};
use crate::feedback::{Event, EventKind, FeedbackStore, Notice, Source};
use crate::git::{Git, GitError, Remote, Repo, RepoRemote, Secret, Upstream};
use crate::github::{Github, GithubError, GithubRepo, Pull, PullComment};
use crate::highlight::{
    Grammar, HighlightStore, Highlighter, Highlights, Installed, LangError, Pack, Span, Tag,
};
use crate::item::{
    ChangedFile, DiffStat, FileStat, Item, ItemGit, ItemId, ItemState, ItemStore, NewItem,
    NewRevision, Pushed, Revision, Submit, submit,
};
use crate::merge::{Landing, MergeBackend, MergeGit, Trial};
use crate::project::{Backend, Project, ProjectName, ProjectStore};
use crate::review::{
    Anchor, Comment, CommentId, NewComment, NewReply, NewReview, Reply, Review, ReviewStore,
};
use crate::settings::SettingsStore;
use crate::store::StoreError;
use crate::triage::{Rule, RuleId, Rules, RulesStore, Tier, Triage, TriageGit};
use crate::viewed::ViewedStore;

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
    fail_unpin: bool,
    /// The paths every trial merge conflicts on, if it does.
    conflicts: Option<Vec<String>>,
    /// Commits of every repo, by ID. Each has at most one parent.
    commits: BTreeMap<String, Commit>,
    /// Branches of every upstream, a remote's under its URL.
    upstream: RefCell<BTreeMap<(PathBuf, String), String>>,
    /// Where each project's staging last pushed a branch to its remote.
    published: RefCell<BTreeMap<(String, String), String>>,
    staging: RefCell<BTreeMap<String, BTreeMap<String, String>>>,
    synced: RefCell<Vec<(String, PathBuf, String)>>,
    fetched: RefCell<Vec<(String, String)>>,
    pins: RefCell<BTreeMap<String, String>>,
    blobs_read: RefCell<Vec<String>>,
    /// Paths `linguist-generated` in every tree.
    generated: BTreeSet<String>,
}

impl Default for FakeGit {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            version: Some("git version 2.50.1".into()),
            fail_sync: false,
            fail_unpin: false,
            conflicts: None,
            commits: BTreeMap::new(),
            upstream: RefCell::default(),
            published: RefCell::default(),
            staging: RefCell::default(),
            synced: RefCell::default(),
            fetched: RefCell::default(),
            pins: RefCell::default(),
            blobs_read: RefCell::default(),
            generated: BTreeSet::new(),
        }
    }
}

impl FakeGit {
    pub fn repo(self, root: &str, branches: &[&str], head: Option<&str>) -> Self {
        let mut git = self.without_repo(root);
        let mut branches: Vec<String> = branches.iter().map(|b| (*b).into()).collect();
        branches.sort();
        git.repos.push(Repo {
            root: root.into(),
            branches,
            head: head.map(Into::into),
            remotes: Vec::new(),
        });
        git
    }

    /// Gives the repo at `root`, already added, a remote.
    pub fn remote(mut self, root: &str, name: &str, url: &str) -> Self {
        let repo = self.repos.iter_mut().find(|r| r.root == Path::new(root));
        let remotes = &mut repo.expect("add the repo first").remotes;
        remotes.push(RepoRemote {
            name: name.into(),
            url: url.into(),
        });
        remotes.sort_by(|a, b| a.name.cmp(&b.name));
        self
    }

    pub fn conflicting(mut self, paths: &[&str]) -> Self {
        self.conflicts = Some(paths.iter().map(|p| (*p).into()).collect());
        self
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

    pub fn failing_unpin(mut self) -> Self {
        self.fail_unpin = true;
        self
    }

    pub fn generated(mut self, paths: &[&str]) -> Self {
        self.generated.extend(paths.iter().map(|p| (*p).to_owned()));
        self
    }

    pub fn version(mut self, output: Option<&str>) -> Self {
        self.version = output.map(Into::into);
        self
    }

    /// Adds commit `id` on top of `parent`, adding a line to each of `files`:
    /// the commit's ID.
    pub fn commit(mut self, id: &str, parent: Option<&str>, subject: &str, files: &[&str]) -> Self {
        let commit = Commit {
            parent: parent.map(Into::into),
            subject: subject.into(),
            files: files.iter().map(|f| (*f).into()).collect(),
        };
        self.commits.insert(id.into(), commit);
        self
    }

    /// `root` is a repo's root or a remote's URL.
    pub fn upstream(self, root: &str, branch: &str, commit: &str) -> Self {
        self.upstream
            .borrow_mut()
            .insert((root.into(), branch.into()), commit.into());
        self
    }

    /// Where an upstream has `branch`, as a push left it.
    pub fn upstream_branch(&self, root: &str, branch: &str) -> Option<String> {
        self.upstream
            .borrow()
            .get(&(root.into(), branch.into()))
            .cloned()
    }

    /// What someone pushing to an upstream outside mergegate does.
    pub fn push_upstream(&self, root: &str, branch: &str, commit: &str) {
        self.upstream
            .borrow_mut()
            .insert((root.into(), branch.into()), commit.into());
    }

    /// What deleting a branch on GitHub, outside mergegate, does.
    pub fn delete_upstream(&self, root: &str, branch: &str) {
        self.upstream
            .borrow_mut()
            .remove(&(root.into(), branch.into()));
    }

    /// What a worker's `git push` does.
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

    pub fn pins(&self) -> BTreeMap<String, String> {
        self.pins.borrow().clone()
    }

    /// Every blob `blobs` was asked for.
    pub fn blobs_read(&self) -> Vec<String> {
        self.blobs_read.borrow().clone()
    }

    /// Newest first.
    fn history(&self, id: &str) -> Vec<String> {
        let mut history = Vec::new();
        let mut next = Some(id.to_owned());
        while let Some(id) = next {
            next = self.commits.get(&id).and_then(|c| c.parent.clone());
            history.push(id);
        }
        history
    }

    /// `path`'s blob at `commit`, empty if it doesn't exist there. A blob's
    /// ID is its content.
    fn blob(&self, commit: &str, path: &str) -> String {
        self.history(commit)
            .iter()
            .rev()
            .filter(|id| {
                self.commits
                    .get(*id)
                    .is_some_and(|c| c.files.iter().any(|f| f == path))
            })
            .map(|id| format!("{id}\n"))
            .collect()
    }

    /// Newest first.
    fn range(&self, base: &str, head: &str) -> Vec<String> {
        self.history(head)
            .into_iter()
            .take_while(|id| id != base)
            .collect()
    }

    fn fetch(
        &self,
        project: &ProjectName,
        upstream: &Upstream,
        target: &str,
    ) -> Result<(), GitError> {
        if self.fail_sync {
            return Err(GitError::Failed("fetch failed".into()));
        }
        let head = self
            .upstream
            .borrow()
            .get(&(root_of(upstream), target.into()))
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
        upstream: &Upstream,
        target: &str,
    ) -> Result<(), GitError> {
        self.fetch(project, upstream, target)?;
        self.synced
            .borrow_mut()
            .push((project.to_string(), root_of(upstream), target.into()));
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
        upstream: &Upstream,
        target: &str,
    ) -> Result<(), GitError> {
        self.fetch(project, upstream, target)?;
        self.fetched
            .borrow_mut()
            .push((project.to_string(), target.into()));
        Ok(())
    }

    fn push_branch(
        &self,
        project: &ProjectName,
        remote: &Remote,
        branch: &str,
        head: &str,
        pushed: Option<&str>,
    ) -> Result<Pushed, GitError> {
        let mut upstream = self.upstream.borrow_mut();
        let mut published = self.published.borrow_mut();
        let key = (PathBuf::from(&remote.url), branch.to_owned());
        let last = (project.to_string(), branch.to_owned());
        let lease = published.get(&last).map(String::as_str).or(pushed);
        match upstream.get(&key) {
            Some(found) if found != head && Some(found.as_str()) != lease => {
                return Ok(Pushed::Moved(found.clone()));
            }
            None if pushed.is_some() => return Ok(Pushed::Gone),
            _ => {}
        }
        upstream.insert(key, head.into());
        published.insert(last, head.into());
        Ok(Pushed::Done)
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
    ) -> Result<Vec<ChangedFile>, GitError> {
        let paths: BTreeSet<&String> = self
            .range(base, head)
            .iter()
            .flat_map(|id| &self.commits[id].files)
            .collect();
        let blob = |commit, path| Some(self.blob(commit, path)).filter(|b| !b.is_empty());
        Ok(paths
            .into_iter()
            .map(|path| ChangedFile {
                path: path.clone(),
                old_path: None,
                old_blob: blob(base, path),
                new_blob: blob(head, path),
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

    fn unpin(&self, _project: &ProjectName, prefix: &str) -> Result<(), GitError> {
        if self.fail_unpin {
            return Err(GitError::Failed("update-ref failed".into()));
        }
        self.pins.borrow_mut().retain(|r, _| !r.starts_with(prefix));
        Ok(())
    }
}

impl MergeGit for FakeGit {
    fn trial_merge(
        &self,
        project: &ProjectName,
        upstream: &Upstream,
        target: &str,
        _head: &str,
    ) -> Result<Trial, GitError> {
        self.fetch(project, upstream, target)?;
        Ok(match &self.conflicts {
            Some(paths) => Trial::Conflict {
                target_commit: self.branch_head(project, target)?.unwrap_or_default(),
                paths: paths.clone(),
            },
            None => Trial::Clean,
        })
    }
}

/// The URL of a remote, as a path, so remotes and repos share one map.
fn root_of(upstream: &Upstream) -> PathBuf {
    match upstream {
        Upstream::Repo(root) => root.clone(),
        Upstream::Remote(remote) => PathBuf::from(&remote.url),
    }
}

impl TriageGit for FakeGit {
    fn generated(
        &self,
        _project: &ProjectName,
        _commit: &str,
        paths: &[&str],
    ) -> Result<Vec<bool>, GitError> {
        Ok(paths.iter().map(|p| self.generated.contains(*p)).collect())
    }
}

impl DiffGit for FakeGit {
    fn blobs(
        &self,
        _project: &ProjectName,
        ids: &[&str],
    ) -> Result<Vec<Option<Vec<u8>>>, GitError> {
        self.blobs_read
            .borrow_mut()
            .extend(ids.iter().map(|id| (*id).to_owned()));
        Ok(ids.iter().map(|id| Some(id.as_bytes().to_vec())).collect())
    }
}

type BlobPair = (Option<String>, Option<String>);

#[derive(Default)]
pub(crate) struct FakeStore {
    projects: BTreeMap<ProjectName, Project>,
    /// Oldest first.
    items: Vec<Item>,
    files: BTreeMap<(ItemId, u32), Vec<FileStat>>,
    revisions: BTreeMap<(ItemId, u32), Revision>,
    diffs: BTreeMap<BlobPair, FileDiff>,
    /// By blob, grammar and version.
    highlights: BTreeMap<(String, String, String), Highlights>,
    viewed: BTreeSet<(ItemId, u32, String)>,
    reviews: Vec<Review>,
    /// Every item's, oldest first.
    comments: Vec<Comment>,
    /// Comment IDs aren't reused, as the store's aren't.
    last_comment: u64,
    /// Every item's, oldest first.
    replies: Vec<Reply>,
    last_reply: u64,
    /// Every item's, oldest first.
    events: Vec<(ItemId, Event)>,
    delivered: BTreeMap<ItemId, u64>,
    agents: BTreeMap<ItemId, Agent>,
    woken: BTreeMap<ItemId, u64>,
    /// An external comment's `(seen_at, pending_at)`.
    external: BTreeMap<CommentId, (u64, Option<u64>)>,
    /// The external comments a batch carried.
    batched: BTreeSet<CommentId>,
    rules: BTreeMap<ProjectName, Rules>,
    clock: u32,
    /// Every item's, oldest first.
    attachments: Vec<(ItemId, Attachment)>,
    settings: BTreeMap<String, String>,
}

impl FakeStore {
    /// `app#1` with `revisions` revisions: r1 changes `README.md` and
    /// `src/login.rs`, each later one adds `src/r<n>.rs`.
    pub fn with_item(revisions: u8) -> Self {
        let mut store = Self::default();
        store
            .insert_project(&Project {
                name: ProjectName::parse("app").unwrap(),
                backend: Backend::Local,
                target: "main".into(),
                upstream: "/src/app".into(),
                github: None,
                bots: Vec::new(),
            })
            .unwrap();
        let mut git = FakeGit::default()
            .commit(&commit(1), None, "Initial", &["README.md"])
            .commit(
                &commit(2),
                Some(&commit(1)),
                "Add login",
                &["src/login.rs", "README.md"],
            )
            .upstream("/src/app", "main", &commit(1));
        for n in 2..=revisions {
            let file = format!("src/r{n}.rs");
            git = git.commit(&commit(n + 1), Some(&commit(n)), "More", &[&file]);
        }
        for n in 1..=revisions {
            git.push("app", "login", &commit(n + 1));
            let request = Submit {
                project: "app".into(),
                branch: "login".into(),
                head: commit(n + 1),
                title: None,
                worktree: None,
                wake: None,
            };
            submit(&mut store, &git, &FakeGithub::default(), request).unwrap();
        }
        store
    }

    pub fn reviews(&self) -> &[Review] {
        &self.reviews
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn set_state(&mut self, id: &ItemId, state: ItemState) {
        self.items.iter_mut().find(|i| i.id == *id).unwrap().state = state;
    }

    fn now(&mut self) -> String {
        self.clock += 1;
        format!("2026-01-01T00:00:{:02}Z", self.clock)
    }

    pub fn diffs(&self) -> &BTreeMap<BlobPair, FileDiff> {
        &self.diffs
    }

    pub fn forget_diffs(&mut self) {
        self.diffs.clear();
    }

    /// Blob, grammar and version of every cached entry.
    pub fn highlighted(&self) -> Vec<(String, String, String)> {
        self.highlights.keys().cloned().collect()
    }

    /// Blobs of every revision of the items `keep` picks.
    fn blobs_of(&self, keep: impl Fn(&ItemId) -> bool) -> BTreeSet<String> {
        self.files
            .iter()
            .filter(|((item, _), _)| keep(item))
            .flat_map(|(_, files)| files)
            .flat_map(|f| [f.file.old_blob.clone(), f.file.new_blob.clone()])
            .flatten()
            .collect()
    }

    fn revision(&mut self, id: &ItemId, number: u32, revision: NewRevision) -> Revision {
        self.files
            .insert((id.clone(), number), revision.files.clone());
        let revision = Revision {
            number,
            stat: DiffStat::of(&revision.files),
            head: revision.head,
            base: revision.base,
            submitted_at: self.now(),
        };
        self.revisions
            .insert((id.clone(), number), revision.clone());
        revision
    }

    /// The fake clock's stamp in milliseconds since the first.
    pub fn millis(stamp: &str) -> u64 {
        let seconds: u64 = stamp
            .strip_prefix("2026-01-01T00:00:")
            .and_then(|s| s.strip_suffix('Z'))
            .and_then(|s| s.parse().ok())
            .unwrap();
        seconds * 1000
    }

    /// An external comment's `(seen_at, pending_at)`.
    pub fn external_times(&self, id: CommentId) -> (u64, Option<u64>) {
        self.external[&id]
    }

    fn push_event(&mut self, id: &ItemId, kind: EventKind) -> Event {
        // Cursors are hub-wide, as the store's are.
        let event = Event {
            cursor: self.events.len() as u64 + 1,
            kind,
        };
        self.events.push((id.clone(), event.clone()));
        event
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

    fn item(&self, id: &ItemId) -> Result<Option<Item>, StoreError> {
        Ok(self.items.iter().find(|i| i.id == *id).cloned())
    }

    fn revisions(&self, id: &ItemId) -> Result<Vec<Revision>, StoreError> {
        Ok(self
            .revisions
            .iter()
            .filter(|((item, _), _)| item == id)
            .map(|(_, revision)| revision.clone())
            .collect())
    }

    fn revision_files(&self, id: &ItemId, revision: u32) -> Result<Vec<FileStat>, StoreError> {
        Ok(self
            .files
            .get(&(id.clone(), revision))
            .cloned()
            .unwrap_or_default())
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
        let id = ItemId::new(item.project, number);
        let revision = self.revision(&id, 1, item.revision);
        let item = Item {
            id,
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
        let revision = self.revision(id, number, revision);
        let item = &mut self.items[index];
        item.title = title.into();
        item.state = state;
        item.revision = revision;
        Ok(item.clone())
    }
}

impl RulesStore for FakeStore {
    fn applied_rules(&self, project: &ProjectName) -> Result<Rules, StoreError> {
        Ok(self.rules.get(project).cloned().unwrap_or_default())
    }

    fn set_applied_rules(
        &mut self,
        project: &ProjectName,
        rules: &Rules,
    ) -> Result<(), StoreError> {
        self.rules.insert(project.clone(), rules.clone());
        Ok(())
    }

    fn set_triage(
        &mut self,
        id: &ItemId,
        revision: u32,
        triage: &[(String, Triage)],
    ) -> Result<(), StoreError> {
        let files = self.files.get_mut(&(id.clone(), revision)).unwrap();
        for (path, triage) in triage {
            let file = files.iter_mut().find(|f| f.file.path == *path).unwrap();
            file.triage = triage.clone();
        }
        let stat = DiffStat::of(files);
        let item = self.items.iter_mut().find(|i| i.id == *id).unwrap();
        if item.revision.number == revision {
            item.revision.stat = stat;
        }
        Ok(())
    }
}

/// Rules with `R1` first, from `(tier, path)` pairs.
pub(crate) fn rules(rules: &[(Tier, &str)]) -> Rules {
    Rules::new(
        (1..)
            .zip(rules)
            .map(|(n, (tier, path))| Rule {
                id: RuleId::new(n),
                tier: *tier,
                path: Some((*path).to_owned()),
                kind: None,
                max_lines: None,
                diff_matches: None,
                reason: format!("rule {n}"),
            })
            .collect(),
    )
    .unwrap()
}

impl DiffStore for FakeStore {
    fn diff(&self, old: Option<&str>, new: Option<&str>) -> Result<Option<FileDiff>, StoreError> {
        Ok(self.diffs.get(&pair(old, new)).cloned())
    }

    fn insert_diff(
        &mut self,
        old: Option<&str>,
        new: Option<&str>,
        diff: &FileDiff,
    ) -> Result<(), StoreError> {
        self.diffs.insert(pair(old, new), diff.clone());
        Ok(())
    }

    fn prune_diffs(&mut self, id: &ItemId) -> Result<(), StoreError> {
        let pairs = |items: &mut dyn Iterator<Item = &(ItemId, u32)>| -> BTreeSet<BlobPair> {
            items
                .flat_map(|key| &self.files[key])
                .map(|f| (f.file.old_blob.clone(), f.file.new_blob.clone()))
                .collect()
        };
        let open: BTreeSet<&ItemId> = self
            .items
            .iter()
            .filter(|i| i.state.is_open())
            .map(|i| &i.id)
            .collect();
        let kept = pairs(&mut self.files.keys().filter(|(item, _)| open.contains(item)));
        let closing = pairs(&mut self.files.keys().filter(|(item, _)| item == id));
        self.diffs
            .retain(|pair, _| !closing.contains(pair) || kept.contains(pair));
        Ok(())
    }
}

impl HighlightStore for FakeStore {
    fn highlights(
        &self,
        blob: &str,
        grammar: &str,
        version: &str,
    ) -> Result<Option<Highlights>, StoreError> {
        Ok(self
            .highlights
            .get(&(blob.into(), grammar.into(), version.into()))
            .cloned())
    }

    fn highlight_lines(
        &self,
        blob: &str,
        grammar: &str,
        version: &str,
        lines: &[u32],
    ) -> Result<Option<Highlights>, StoreError> {
        let Some(all) = self.highlights(blob, grammar, version)? else {
            return Ok(None);
        };
        // As the sqlite store: none after the last wanted line.
        let last = lines.last().copied().unwrap_or(0);
        let numbered = (1..=last).zip(all.lines);
        Ok(Some(Highlights {
            lines: numbered
                .map(|(n, spans)| {
                    if lines.contains(&n) {
                        spans
                    } else {
                        Vec::new()
                    }
                })
                .collect(),
        }))
    }

    fn has_highlights(&self, blob: &str, grammar: &str, version: &str) -> Result<bool, StoreError> {
        Ok(self.highlights(blob, grammar, version)?.is_some())
    }

    fn insert_highlights(
        &mut self,
        blob: &str,
        grammar: &str,
        version: &str,
        highlights: &Highlights,
    ) -> Result<(), StoreError> {
        self.highlights.insert(
            (blob.into(), grammar.into(), version.into()),
            highlights.clone(),
        );
        Ok(())
    }

    fn retain_highlights(&mut self, version: &str) -> Result<(), StoreError> {
        self.highlights.retain(|(_, _, v), _| v == version);
        Ok(())
    }

    fn prune_highlights(&mut self, id: &ItemId) -> Result<(), StoreError> {
        let open: BTreeSet<ItemId> = self
            .items
            .iter()
            .filter(|i| i.state.is_open())
            .map(|i| i.id.clone())
            .collect();
        let kept = self.blobs_of(|item| open.contains(item));
        let closing = self.blobs_of(|item| item == id);
        self.highlights
            .retain(|(blob, _, _), _| !closing.contains(blob) || kept.contains(blob));
        Ok(())
    }
}

/// Packs of one grammar each, named after their pack, with `name` as its
/// extension. Each line highlights as one keyword span over its text.
#[derive(Default)]
pub(crate) struct FakeHighlighter {
    pub packs: Vec<Pack>,
}

impl FakeHighlighter {
    pub fn with(names: &[(&str, &str)]) -> Self {
        Self {
            packs: names.iter().map(|(n, v)| pack(n, v)).collect(),
        }
    }
}

/// What a `FakeHighlighter::with(names)` has installed.
pub(crate) fn installed(names: &[(&str, &str)]) -> Installed {
    FakeHighlighter::with(names).installed()
}

pub(crate) fn pack(name: &str, version: &str) -> Pack {
    Pack {
        name: name.into(),
        version: version.into(),
        grammars: vec![Grammar {
            name: name.into(),
            extensions: vec![name.into()],
            filenames: Vec::new(),
            aliases: Vec::new(),
        }],
    }
}

impl Highlighter for FakeHighlighter {
    /// Versioned by each pack's name and version, as `rs@1,md@2`.
    fn installed(&self) -> Installed {
        let version = self
            .packs
            .iter()
            .map(|p| format!("{}@{}", p.name, p.version));
        Installed {
            packs: self.packs.clone(),
            version: version.collect::<Vec<_>>().join(","),
        }
    }

    fn install(&self, from: &Path) -> Result<Vec<Pack>, LangError> {
        Err(LangError::Empty(from.into()))
    }

    fn remove(&self, name: &str) -> Result<Pack, LangError> {
        Err(LangError::NotInstalled(name.into()))
    }

    fn highlight(&self, grammar: &str, text: &str) -> Option<Highlights> {
        self.packs.iter().find(|p| p.name == grammar)?;
        let lines = text
            .split('\n')
            .map(|line| match line.encode_utf16().count() as u32 {
                0 => Vec::new(),
                len => vec![Span {
                    start: 0,
                    len,
                    tag: Tag::Keyword,
                }],
            })
            .collect();
        Some(Highlights { lines })
    }
}

impl ViewedStore for FakeStore {
    fn viewed(&self, id: &ItemId, revision: u32) -> Result<BTreeSet<String>, StoreError> {
        Ok(self
            .viewed
            .iter()
            .filter(|(i, r, _)| i == id && *r == revision)
            .map(|(_, _, path)| path.clone())
            .collect())
    }

    fn set_viewed(
        &mut self,
        id: &ItemId,
        revision: u32,
        path: &str,
        viewed: bool,
    ) -> Result<(), StoreError> {
        let key = (id.clone(), revision, path.to_owned());
        if viewed {
            self.viewed.insert(key);
        } else {
            self.viewed.remove(&key);
        }
        Ok(())
    }
}

impl AttachmentStore for FakeStore {
    fn attachments(&self, id: &ItemId) -> Result<Vec<Attachment>, StoreError> {
        let mut attachments: Vec<Attachment> = self
            .attachments
            .iter()
            .filter(|(i, _)| i == id)
            .map(|(_, a)| a.clone())
            .collect();
        attachments.sort_by_key(|a| a.revision);
        Ok(attachments)
    }

    fn add_attachment(
        &mut self,
        id: &ItemId,
        attachment: NewAttachment<'_>,
    ) -> Result<Attachment, StoreError> {
        let attachment = Attachment {
            revision: attachment.revision,
            name: attachment.name.into(),
            media: attachment.media,
            size: attachment.bytes.len() as u64,
            // Not a real hash: the fake only needs one per content.
            sha256: attachment
                .bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        };
        self.attachments.push((id.clone(), attachment.clone()));
        Ok(attachment)
    }

    fn attachment_media(&self, sha256: &str) -> Result<Option<Media>, StoreError> {
        Ok(self
            .attachments
            .iter()
            .find(|(_, a)| a.sha256 == sha256)
            .map(|(_, a)| a.media))
    }

    /// The fake keeps no close times: a closed item's attachments go only
    /// with a zero TTL.
    fn prune_attachments(&mut self, ttl: std::time::Duration) -> Result<Vec<ItemId>, StoreError> {
        if !ttl.is_zero() {
            return Ok(Vec::new());
        }
        let items = &self.items;
        let mut pruned = Vec::new();
        self.attachments.retain(|(id, _)| {
            let open = items
                .iter()
                .any(|item| &item.id == id && item.state.is_open());
            if !open && !pruned.contains(id) {
                pruned.push(id.clone());
            }
            open
        });
        Ok(pruned)
    }
}

impl SettingsStore for FakeStore {
    fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self.settings.get(key).cloned())
    }

    fn set_setting(&mut self, key: &str, value: &str) -> Result<(), StoreError> {
        self.settings.insert(key.into(), value.into());
        Ok(())
    }
}

impl ReviewStore for FakeStore {
    fn insert_review(&mut self, review: NewReview, state: ItemState) -> Result<Review, StoreError> {
        let id = self.reviews.len() as u64 + 1;
        let mut comments = Vec::new();
        for comment in &mut self.comments {
            if comment.item == review.item && comment.is_draft() {
                comment.review = Some(id);
                comments.push(comment.clone());
            }
        }
        let mut replies = Vec::new();
        for reply in &mut self.replies {
            if reply.item == review.item && reply.is_draft() {
                reply.review = Some(id);
                replies.push(reply.clone());
            }
        }
        let review = Review {
            id,
            item: review.item,
            revision: review.revision,
            verdict: review.verdict,
            summary: review.summary,
            comments,
            replies,
            submitted_at: self.now(),
        };
        self.set_state(&review.item, state);
        self.reviews.push(review.clone());
        // Cursors are hub-wide, as the store's are.
        let event = Event {
            cursor: self.events.len() as u64 + 1,
            kind: EventKind::ReviewSubmitted(review.clone()),
        };
        self.events.push((review.item.clone(), event));
        Ok(review)
    }

    fn insert_draft(&mut self, comment: NewComment) -> Result<Comment, StoreError> {
        self.last_comment += 1;
        let comment = Comment {
            id: CommentId(self.last_comment),
            item: comment.item,
            revision: comment.revision,
            anchor: Some(Anchor {
                path: comment.path,
                side: comment.side,
                start_line: comment.start_line,
                line: comment.line,
                quote: comment.quote,
            }),
            body: comment.body,
            review: None,
            external: None,
            created_at: self.now(),
        };
        self.comments.push(comment.clone());
        Ok(comment)
    }

    fn comments(&self, id: &ItemId) -> Result<Vec<Comment>, StoreError> {
        Ok(self
            .comments
            .iter()
            .filter(|c| c.item == *id)
            .cloned()
            .collect())
    }

    fn set_draft_body(&mut self, id: CommentId, body: &str) -> Result<(), StoreError> {
        if let Some(draft) = self
            .comments
            .iter_mut()
            .find(|c| c.id == id && c.is_draft())
        {
            draft.body = body.into();
        }
        Ok(())
    }

    fn delete_draft(&mut self, id: CommentId) -> Result<(), StoreError> {
        self.comments.retain(|c| c.id != id || !c.is_draft());
        Ok(())
    }

    fn comment(&self, id: CommentId) -> Result<Option<Comment>, StoreError> {
        Ok(self.comments.iter().find(|c| c.id == id).cloned())
    }

    fn insert_reply(&mut self, reply: NewReply) -> Result<Reply, StoreError> {
        self.last_reply += 1;
        let reply = Reply {
            id: self.last_reply,
            comment: reply.comment,
            item: reply.item,
            author: reply.author,
            status: reply.status,
            body: reply.body,
            revision: reply.revision,
            review: None,
            overrides: false,
            created_at: self.now(),
        };
        self.replies.push(reply.clone());
        Ok(reply)
    }

    fn reply(&self, id: u64) -> Result<Option<Reply>, StoreError> {
        Ok(self.replies.iter().find(|r| r.id == id).cloned())
    }

    fn set_draft_reply_body(&mut self, id: u64, body: &str) -> Result<(), StoreError> {
        if let Some(draft) = self.replies.iter_mut().find(|r| r.id == id && r.is_draft()) {
            draft.body = body.into();
        }
        Ok(())
    }

    fn delete_draft_reply(&mut self, id: u64) -> Result<(), StoreError> {
        self.replies.retain(|r| r.id != id || !r.is_draft());
        Ok(())
    }

    fn replies(&self, id: &ItemId) -> Result<Vec<Reply>, StoreError> {
        Ok(self
            .replies
            .iter()
            .filter(|r| r.item == *id)
            .cloned()
            .collect())
    }
}

impl FeedbackStore for FakeStore {
    fn events(&self, id: &ItemId, after: u64) -> Result<Vec<Event>, StoreError> {
        Ok(self
            .events
            .iter()
            .filter(|(item, event)| item == id && event.cursor > after)
            .map(|(_, event)| event.clone())
            .collect())
    }

    fn last_cursor(&self, id: &ItemId) -> Result<u64, StoreError> {
        Ok(self.events(id, 0)?.last().map_or(0, |e| e.cursor))
    }

    fn delivered(&self, id: &ItemId) -> Result<u64, StoreError> {
        Ok(self.delivered.get(id).copied().unwrap_or(0))
    }

    fn set_delivered(&mut self, id: &ItemId, cursor: u64) -> Result<(), StoreError> {
        self.delivered.insert(id.clone(), cursor);
        Ok(())
    }

    fn notify(
        &mut self,
        id: &ItemId,
        state: ItemState,
        notice: Notice,
    ) -> Result<Event, StoreError> {
        self.set_state(id, state);
        let event = Event {
            cursor: self.events.len() as u64 + 1,
            kind: EventKind::Notice(notice),
        };
        self.events.push((id.clone(), event.clone()));
        Ok(event)
    }
}

impl ExternalStore for FakeStore {
    fn insert_external(&mut self, comment: NewExternal, now: u64) -> Result<Comment, StoreError> {
        self.last_comment += 1;
        let comment = Comment {
            id: CommentId(self.last_comment),
            item: comment.item,
            revision: comment.revision,
            anchor: comment.anchor,
            body: comment.body,
            review: None,
            external: Some(External {
                kind: comment.kind,
                github_id: comment.github_id,
                login: comment.login,
                bot: comment.bot,
                url: comment.url,
                path: comment.path,
                pending: true,
                agreed_reply: None,
            }),
            created_at: self.now(),
        };
        self.comments.push(comment.clone());
        self.external.insert(comment.id, (now, Some(now)));
        Ok(comment)
    }

    fn edit_external(&mut self, id: CommentId, body: &str, now: u64) -> Result<(), StoreError> {
        let comment = self.comments.iter_mut().find(|c| c.id == id).unwrap();
        comment.body = body.into();
        comment.external.as_mut().unwrap().pending = true;
        let times = self.external.get_mut(&id).unwrap();
        *times = (now, Some(times.1.unwrap_or(now)));
        Ok(())
    }

    fn held(&self, id: &ItemId) -> Result<Option<Held>, StoreError> {
        let item = self.item(id)?.unwrap();
        let pushed = Self::millis(&item.revision.submitted_at);
        let mut held: Option<Held> = None;
        let mut heard = Vec::new();
        for comment in self.comments.iter().filter(|c| c.item == *id) {
            let Some(external) = &comment.external else {
                continue;
            };
            let (seen, pending) = self.external[&comment.id];
            if external.bot && seen >= pushed && !heard.contains(&external.login) {
                heard.push(external.login.clone());
            }
            if let Some(pending) = pending {
                let held = held.get_or_insert(Held {
                    first: pending,
                    last: seen,
                    pushed,
                    heard: Vec::new(),
                });
                held.first = held.first.min(pending);
                held.last = held.last.max(seen);
            }
        }
        Ok(held.map(|held| Held { heard, ..held }))
    }

    fn release(&mut self, id: &ItemId, revision: u32) -> Result<Option<Event>, StoreError> {
        let mut comments = Vec::new();
        for comment in self.comments.iter_mut().filter(|c| c.item == *id) {
            let Some(times) = self.external.get_mut(&comment.id) else {
                continue;
            };
            if times.1.take().is_none() {
                continue;
            }
            comment.external.as_mut().unwrap().pending = false;
            comments.push(Delivered {
                comment: comment.clone(),
                edited: !self.batched.insert(comment.id),
            });
        }
        if comments.is_empty() {
            return Ok(None);
        }
        let batch = Batch { revision, comments };
        Ok(Some(
            self.push_event(id, EventKind::ExternalComments(batch)),
        ))
    }

    fn set_agreed(&mut self, id: CommentId, reply: u64) -> Result<(), StoreError> {
        let comment = self.comments.iter_mut().find(|c| c.id == id).unwrap();
        comment.external.as_mut().unwrap().agreed_reply = Some(reply);
        Ok(())
    }

    fn insert_override(&mut self, reply: NewReply) -> Result<(Reply, Event), StoreError> {
        let item = reply.item.clone();
        let mut reply = self.insert_reply(reply)?;
        reply.overrides = true;
        self.replies.last_mut().unwrap().overrides = true;
        let comment = self.comment(reply.comment)?.unwrap();
        let kind = EventKind::Override(Box::new(Override {
            comment,
            reply: reply.clone(),
        }));
        let event = self.push_event(&item, kind);
        Ok((reply, event))
    }
}

impl AgentStore for FakeStore {
    fn agent(&self, id: &ItemId) -> Result<Agent, StoreError> {
        Ok(self.agents.get(id).cloned().unwrap_or_default())
    }

    fn set_agent(&mut self, id: &ItemId, agent: &Agent) -> Result<(), StoreError> {
        self.agents.insert(id.clone(), agent.clone());
        Ok(())
    }

    fn due_wakes(&self) -> Result<Vec<DueWake>, StoreError> {
        let mut due = Vec::new();
        for (id, agent) in &self.agents {
            let Some(command) = &agent.wake else {
                continue;
            };
            let seen = self
                .delivered(id)?
                .max(self.woken.get(id).copied().unwrap_or(0));
            let cursor = self
                .events(id, 0)?
                .iter()
                .rev()
                .find(|e| e.source() != Source::Agent)
                .map_or(0, |e| e.cursor);
            if cursor > seen {
                due.push(DueWake {
                    item: id.clone(),
                    command: command.clone(),
                    worktree: agent.worktree.clone(),
                    cursor,
                });
            }
        }
        Ok(due)
    }

    fn set_woken(&mut self, id: &ItemId, cursor: u64) -> Result<(), StoreError> {
        self.woken.insert(id.clone(), cursor);
        Ok(())
    }
}

fn pair(old: Option<&str>, new: Option<&str>) -> BlobPair {
    (old.map(Into::into), new.map(Into::into))
}

/// A merge backend that lands the way the test scripted, oldest first, and
/// lands everything after the script runs out as merged at c9.
pub(crate) struct FakeBackend {
    kind: Backend,
    script: RefCell<Vec<Result<Landing, GitError>>>,
    landed: RefCell<Vec<(String, String)>>,
}

impl FakeBackend {
    pub fn new(kind: Backend) -> Self {
        Self {
            kind,
            script: RefCell::default(),
            landed: RefCell::default(),
        }
    }

    pub fn then(self, landing: Result<Landing, GitError>) -> Self {
        self.script.borrow_mut().push(landing);
        self
    }

    /// The `(item, message)` of every land, oldest first.
    pub fn landed(&self) -> Vec<(String, String)> {
        self.landed.borrow().clone()
    }
}

impl MergeBackend for FakeBackend {
    fn kind(&self) -> Backend {
        self.kind
    }

    fn land(&self, _project: &Project, item: &Item, message: &str) -> Result<Landing, GitError> {
        self.landed
            .borrow_mut()
            .push((item.id.to_string(), message.to_owned()));
        let mut script = self.script.borrow_mut();
        if script.is_empty() {
            return Ok(Landing::Merged { commit: commit(9) });
        }
        script.remove(0)
    }
}

/// GitHub as the operator's account sees it: every repository is theirs to
/// push to unless denied. Git reaches each at `https://github.com/<repo>.git`.
pub(crate) struct FakeGithub {
    logged_in: bool,
    denied: BTreeSet<GithubRepo>,
    refusal: RefCell<Option<String>>,
    pulls: RefCell<Vec<FakePull>>,
    deleted: RefCell<Vec<(GithubRepo, String)>>,
    /// Every pull request's, oldest first.
    comments: RefCell<Vec<(GithubRepo, u64, PullComment)>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FakePull {
    pub repo: GithubRepo,
    pub number: u64,
    pub branch: String,
    pub target: String,
    pub title: String,
    pub state: PullState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PullState {
    Open,
    Closed,
    /// Squash-merged at this head, with this title.
    Merged {
        head: String,
        title: String,
    },
}

impl Default for FakeGithub {
    fn default() -> Self {
        Self {
            logged_in: true,
            denied: BTreeSet::new(),
            refusal: RefCell::default(),
            pulls: RefCell::default(),
            deleted: RefCell::default(),
            comments: RefCell::default(),
        }
    }
}

impl FakeGithub {
    pub fn logged_out(mut self) -> Self {
        self.logged_in = false;
        self
    }

    pub fn denying(mut self, repo: &str) -> Self {
        self.denied.insert(GithubRepo::parse(repo).unwrap());
        self
    }

    /// Every merge from now on is refused with `reason`.
    pub fn refuse(&self, reason: &str) {
        self.refusal.replace(Some(reason.into()));
    }

    /// What someone opening a pull request on GitHub does.
    pub fn open(&self, repo: &str, branch: &str, title: &str) -> Pull {
        let repo = GithubRepo::parse(repo).unwrap();
        self.open_pull(&repo, branch, "main", title).unwrap()
    }

    pub fn pulls(&self) -> Vec<FakePull> {
        self.pulls.borrow().clone()
    }

    pub fn deleted(&self) -> Vec<(GithubRepo, String)> {
        self.deleted.borrow().clone()
    }

    /// What someone commenting on the pull request does; one with the
    /// comment's kind and ID already there is edited.
    pub fn comment(&self, repo: &str, number: u64, comment: PullComment) {
        let repo = GithubRepo::parse(repo).unwrap();
        let mut comments = self.comments.borrow_mut();
        match comments.iter_mut().find(|(r, n, c)| {
            *r == repo && *n == number && c.kind == comment.kind && c.id == comment.id
        }) {
            Some((_, _, known)) => *known = comment,
            None => comments.push((repo, number, comment)),
        }
    }

    fn signed_in(&self) -> Result<(), GithubError> {
        if self.logged_in {
            Ok(())
        } else {
            Err(GithubError::NotLoggedIn("gh is not logged in".into()))
        }
    }

    fn pull<T>(
        &self,
        repo: &GithubRepo,
        number: u64,
        then: impl FnOnce(&mut FakePull) -> Result<T, GithubError>,
    ) -> Result<T, GithubError> {
        self.signed_in()?;
        let mut pulls = self.pulls.borrow_mut();
        let pull = pulls
            .iter_mut()
            .find(|p| p.repo == *repo && p.number == number)
            .ok_or_else(|| GithubError::Failed(format!("no pull request {number}")))?;
        then(pull)
    }
}

fn pull_of(pull: &FakePull) -> Pull {
    Pull {
        number: pull.number,
        url: format!("https://github.com/{}/pull/{}", pull.repo, pull.number),
    }
}

impl Github for FakeGithub {
    fn remote(&self, repo: &GithubRepo) -> Result<Remote, GithubError> {
        self.signed_in()?;
        Ok(Remote {
            url: format!("https://github.com/{repo}.git"),
            auth: Secret::new("Authorization: Basic dGVzdA==".into()),
        })
    }

    fn check_access(&self, repo: &GithubRepo) -> Result<(), GithubError> {
        self.signed_in()?;
        if self.denied.contains(repo) {
            return Err(GithubError::NoAccess(repo.clone()));
        }
        Ok(())
    }

    fn find_pull(
        &self,
        repo: &GithubRepo,
        branch: &str,
        target: &str,
    ) -> Result<Option<Pull>, GithubError> {
        self.signed_in()?;
        Ok(self
            .pulls
            .borrow()
            .iter()
            .find(|p| {
                p.repo == *repo
                    && p.branch == branch
                    && p.target == target
                    && p.state == PullState::Open
            })
            .map(pull_of))
    }

    fn open_pull(
        &self,
        repo: &GithubRepo,
        branch: &str,
        target: &str,
        title: &str,
    ) -> Result<Pull, GithubError> {
        self.check_access(repo)?;
        let mut pulls = self.pulls.borrow_mut();
        let pull = FakePull {
            repo: repo.clone(),
            number: pulls.len() as u64 + 1,
            branch: branch.into(),
            target: target.into(),
            title: title.into(),
            state: PullState::Open,
        };
        pulls.push(pull.clone());
        Ok(pull_of(&pull))
    }

    fn retitle_pull(&self, repo: &GithubRepo, number: u64, title: &str) -> Result<(), GithubError> {
        self.pull(repo, number, |pull| {
            pull.title = title.into();
            Ok(())
        })
    }

    fn squash_merge(
        &self,
        repo: &GithubRepo,
        number: u64,
        title: &str,
        head: &str,
    ) -> Result<String, GithubError> {
        let refusal = self.refusal.borrow().clone();
        self.pull(repo, number, |pull| {
            if let PullState::Merged { head: merged, .. } = &pull.state
                && merged == head
            {
                return Ok(commit(8));
            }
            if let Some(reason) = refusal {
                return Err(GithubError::Refused(reason));
            }
            pull.state = PullState::Merged {
                head: head.into(),
                title: title.into(),
            };
            Ok(commit(8))
        })
    }

    fn merged(
        &self,
        repo: &GithubRepo,
        number: u64,
        head: &str,
    ) -> Result<Option<String>, GithubError> {
        self.pull(repo, number, |pull| {
            Ok(
                matches!(&pull.state, PullState::Merged { head: merged, .. } if merged == head)
                    .then(|| commit(8)),
            )
        })
    }

    fn close_pull(&self, repo: &GithubRepo, number: u64) -> Result<(), GithubError> {
        self.pull(repo, number, |pull| {
            pull.state = PullState::Closed;
            Ok(())
        })
    }

    fn delete_branch(&self, repo: &GithubRepo, branch: &str) -> Result<(), GithubError> {
        self.signed_in()?;
        self.deleted
            .borrow_mut()
            .push((repo.clone(), branch.into()));
        Ok(())
    }

    fn viewer(&self) -> Result<String, GithubError> {
        self.signed_in()?;
        Ok("operator".into())
    }

    fn pull_comments(
        &self,
        repo: &GithubRepo,
        number: u64,
        _fresh: bool,
    ) -> Result<Vec<PullComment>, GithubError> {
        self.signed_in()?;
        Ok(self
            .comments
            .borrow()
            .iter()
            .filter(|(r, n, _)| r == repo && *n == number)
            .map(|(_, _, c)| c.clone())
            .collect())
    }
}
