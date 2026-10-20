//! A GitHub on localhost: the REST endpoints the hub calls, under `/api`, and
//! git over HTTP for its repositories, both signed in to with one token.
//! A fake `gh` hands that token out, so the hub signs in the way it does on
//! the operator's machine.
//!
//! Point a hub at it with [`FakeGithub::env`]. Its pull requests' pages are
//! plain HTML at their `html_url`, for a person following the link.

mod comments;
mod git_http;
mod page;
mod reactions;
mod rest;
mod server;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tempfile::TempDir;

use crate::repo::git;

pub use comments::{Comment, CommentKind, Inline, NewComment};

pub struct FakeGithub {
    shared: Arc<Shared>,
    dir: TempDir,
    addr: String,
}

/// What the server threads and the test share.
struct Shared {
    state: Mutex<State>,
    /// Bare repositories, as `{owner}/{repo}.git`.
    repos: PathBuf,
    /// The fake `gh` reads its token here; no file is no login.
    token_file: PathBuf,
    web: String,
    stopping: AtomicBool,
}

struct State {
    token: String,
    /// By `owner/repo`: whether the signed-in account may push.
    repos: BTreeMap<String, bool>,
    pulls: Vec<Pull>,
    /// By `owner/repo`: how many pull requests were opened elsewhere, which
    /// the numbers of the hub's come after.
    opened_elsewhere: BTreeMap<String, u64>,
    refusal: Option<String>,
    /// Merges wait until this is cleared.
    holding_merges: bool,
    requests: Vec<String>,
    comments: Vec<Comment>,
    last_comment: u64,
    /// IDs of comments lists leave out though they are there.
    unlisted: BTreeSet<u64>,
    /// Who `GET /user` says is signed in.
    viewer: String,
    /// The most a list page holds, whatever `per_page` asks for.
    page_size: usize,
    /// How many list requests were answered 304.
    not_modified: usize,
    reactions: Vec<(reactions::Reaction, u64)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            token: String::new(),
            repos: BTreeMap::new(),
            pulls: Vec::new(),
            opened_elsewhere: BTreeMap::new(),
            refusal: None,
            holding_merges: false,
            requests: Vec::new(),
            comments: Vec::new(),
            last_comment: 0,
            unlisted: BTreeSet::new(),
            viewer: "operator".into(),
            page_size: 100,
            not_modified: 0,
            reactions: Vec::new(),
        }
    }
}

/// A pull request as the fake keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pull {
    pub repo: String,
    pub number: u64,
    pub branch: String,
    pub base: String,
    pub title: String,
    pub body: String,
    pub draft: bool,
    pub state: PullState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullState {
    Open,
    Closed,
    /// Squashed into `commit` while the branch was at `head`.
    Merged {
        head: String,
        commit: String,
    },
}

impl FakeGithub {
    pub fn start() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("mg-github-")
            .tempdir()
            .expect("create fake GitHub dir");
        let root = dir.path().canonicalize().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake GitHub");
        let addr = listener.local_addr().unwrap().to_string();
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            repos: root.join("repos"),
            token_file: root.join("token"),
            web: format!("http://{addr}"),
            stopping: AtomicBool::new(false),
        });
        std::fs::create_dir_all(&shared.repos).unwrap();
        write_gh(&root.join("bin"), &shared.token_file);
        let github = Self { shared, dir, addr };
        github.log_in("gho_first");
        let shared = github.shared.clone();
        std::thread::spawn(move || server::serve(listener, &shared));
        github
    }

    /// The variables that point a hub, and the `gh` it runs, at this fake.
    pub fn env(&self) -> Vec<(String, String)> {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.bin()];
        paths.extend(std::env::split_paths(&path));
        let path = std::env::join_paths(paths).unwrap();
        vec![
            ("PATH".into(), path.to_str().unwrap().into()),
            ("MG_GITHUB_API_URL".into(), self.api_url()),
            ("MG_GITHUB_URL".into(), self.web_url()),
        ]
    }

    pub fn api_url(&self) -> String {
        format!("{}/api", self.shared.web)
    }

    pub fn web_url(&self) -> String {
        self.shared.web.clone()
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Where repository `owner/repo` is kept, as `owner/repo.git`. Bare
    /// clones put here are served too, for processes that can't call
    /// [`FakeGithub::create_repo`].
    pub fn repos(&self) -> &Path {
        &self.shared.repos
    }

    /// Where the fake `gh` is.
    pub fn bin(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap().join("bin")
    }

    /// The fake `gh`.
    pub fn gh(&self) -> PathBuf {
        self.bin().join("gh")
    }

    /// How git reaches `repo` signed in: its URL, and the header to send.
    pub fn remote(&self, repo: &str) -> (String, String) {
        use base64::Engine;
        let token = self.state().token.clone();
        let basic =
            base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{token}"));
        (
            format!("{}/{repo}.git", self.shared.web),
            format!("Authorization: Basic {basic}"),
        )
    }

    /// `gh` hands out `token` from now on, and only it signs in.
    pub fn log_in(&self, token: &str) {
        std::fs::write(&self.shared.token_file, token).unwrap();
        self.state().token = token.into();
    }

    /// `gh auth token` fails, and the old token no longer signs in.
    pub fn log_out(&self) {
        let _ = std::fs::remove_file(&self.shared.token_file);
        self.state().token = String::new();
    }

    /// `owner/repo`, holding a bare clone of `source`'s branches. The
    /// signed-in account may push to it.
    pub fn create_repo(&self, repo: &str, source: &Path) {
        let path = self.repo_path(repo);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        git(
            source,
            [
                OsStr::new("clone"),
                OsStr::new("--quiet"),
                OsStr::new("--bare"),
                source.as_os_str(),
                path.as_os_str(),
            ],
        );
        self.state().repos.insert(repo.into(), true);
    }

    /// The signed-in account may read `repo` but not push to it.
    pub fn deny_push(&self, repo: &str) {
        self.state().repos.insert(repo.into(), false);
    }

    /// Merges fail with `reason`, as branch protection makes them; `None`
    /// lets them through again.
    pub fn refuse_merges(&self, reason: Option<&str>) {
        self.state().refusal = reason.map(Into::into);
    }

    /// Merges wait at GitHub, as a slow one does, until `false` lets them
    /// through. One waiting is in [`Self::requests`] already.
    pub fn hold_merges(&self, holding: bool) {
        self.state().holding_merges = holding;
    }

    /// Takes the next `count` numbers of `repo`'s pull requests, as ones
    /// opened without the hub would, so the hub's own numbers and its pull
    /// requests' part ways.
    pub fn open_elsewhere(&self, repo: &str, count: u64) {
        *self
            .state()
            .opened_elsewhere
            .entry(repo.into())
            .or_default() += count;
    }

    /// Squash-merges pull request `pull` of `repo` from GitHub's page, as
    /// someone there does without the hub; the commit it landed as.
    #[track_caller]
    pub fn merge_on_github(&self, repo: &str, pull: u64) -> String {
        let reply = rest::act_elsewhere(&self.shared, repo, pull, "merge");
        assert_eq!(
            reply.status,
            200,
            "{}",
            String::from_utf8_lossy(&reply.body)
        );
        let body: serde_json::Value = serde_json::from_slice(&reply.body).unwrap();
        body["sha"].as_str().unwrap().to_owned()
    }

    /// Closes pull request `pull` of `repo` unmerged from GitHub's page.
    #[track_caller]
    pub fn close_on_github(&self, repo: &str, pull: u64) {
        let reply = rest::act_elsewhere(&self.shared, repo, pull, "close");
        assert_eq!(
            reply.status,
            200,
            "{}",
            String::from_utf8_lossy(&reply.body)
        );
    }

    pub fn pulls(&self) -> Vec<Pull> {
        self.state().pulls.clone()
    }

    /// Every request so far, as `METHOD /path`, query left out.
    pub fn requests(&self) -> Vec<String> {
        self.state().requests.clone()
    }

    /// Posts `comment` on pull request `pull` of `repo`, an inline one on
    /// its branch's head; its ID.
    pub fn comment(&self, repo: &str, pull: u64, comment: NewComment) -> u64 {
        let mut state = self.state();
        let head = self.shared.pull_head(&state, repo, pull);
        comments::add(&mut state, repo, pull, head, comment)
    }

    /// Changes comment `id`'s body, as its author editing it.
    pub fn edit_comment(&self, id: u64, body: &str) {
        assert!(
            comments::edit(&mut self.state(), id, body),
            "no comment {id}"
        );
    }

    /// Deletes comment `id`, as its author does.
    pub fn delete_comment(&self, id: u64) {
        let mut state = self.state();
        let before = state.comments.len();
        state.comments.retain(|c| c.id != id);
        assert!(state.comments.len() < before, "no comment {id}");
    }

    /// Lists leave comment `id` out though GitHub still has it, as a list
    /// read while an earlier comment is deleted can skip one across a page.
    pub fn unlist_comment(&self, id: u64) {
        self.state().unlisted.insert(id);
    }

    pub fn comments(&self) -> Vec<Comment> {
        self.state().comments.clone()
    }

    /// `login` reacts `content`, as in `+1` or `eyes`, on pull request
    /// `pull`'s description; with `on` false it takes the reaction back.
    pub fn react(&self, repo: &str, pull: u64, login: &str, content: &str, on: bool) {
        let reaction = reactions::Reaction {
            repo: repo.into(),
            pull,
            login: login.into(),
            content: content.into(),
        };
        reactions::set(&mut self.state(), reaction, on);
    }

    /// `GET /user` says `login` is signed in.
    pub fn set_viewer(&self, login: &str) {
        self.state().viewer = login.into();
    }

    /// Lists come `size` items a page at most.
    pub fn page_size(&self, size: usize) {
        self.state().page_size = size;
    }

    /// How many list requests were answered 304, nothing having changed.
    pub fn not_modified(&self) -> usize {
        self.state().not_modified
    }

    /// Runs git in `repo`'s bare repository and returns its stdout.
    #[track_caller]
    pub fn git<I, S>(&self, repo: &str, args: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        git(&self.repo_path(repo), args)
    }

    /// The commit `rev` points at in `repo`, if any.
    pub fn rev(&self, repo: &str, rev: &str) -> Option<String> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--verify", "--quiet"])
            .arg(format!("{rev}^{{commit}}"))
            .current_dir(self.repo_path(repo))
            .output()
            .unwrap();
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn repo_path(&self, repo: &str) -> PathBuf {
        self.shared.repo_path(repo)
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.shared.state()
    }
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn repo_path(&self, repo: &str) -> PathBuf {
        self.repos.join(format!("{repo}.git"))
    }

    /// What pull request `pull` of `repo` is at: its branch's head, or the
    /// head it was merged at.
    fn pull_head(&self, state: &State, repo: &str, pull: u64) -> Option<String> {
        let pull = state
            .pulls
            .iter()
            .find(|p| p.repo == repo && p.number == pull)?;
        match &pull.state {
            PullState::Merged { head, .. } => Some(head.clone()),
            _ => self.head_of(repo, &pull.branch),
        }
    }

    fn head_of(&self, repo: &str, branch: &str) -> Option<String> {
        if branch.is_empty() {
            return None;
        }
        let rev = format!("refs/heads/{branch}^{{commit}}");
        self.bare(repo, &["rev-parse", "--verify", "--quiet", &rev], &[])
    }

    /// Runs git in `repo`'s bare repository; its trimmed stdout if it
    /// succeeded.
    fn bare(&self, repo: &str, args: &[&str], vars: &[(&str, &str)]) -> Option<String> {
        let mut cmd = std::process::Command::new("git");
        for (key, _) in std::env::vars_os() {
            if key.as_encoded_bytes().starts_with(b"GIT_") {
                cmd.env_remove(key);
            }
        }
        let output = cmd
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .envs(vars.iter().copied())
            .arg("--git-dir")
            .arg(self.repo_path(repo))
            .args(args)
            .output()
            .expect("run git");
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    /// Whether the signed-in account may push to `repo`, if it exists. A
    /// bare repository put under [`FakeGithub::repos`] from outside counts as
    /// one it may push to.
    fn can_push(&self, state: &State, repo: &str) -> Option<bool> {
        let known = state.repos.get(repo).copied();
        known.or_else(|| self.repo_path(repo).is_dir().then_some(true))
    }
}

impl Drop for FakeGithub {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        // Wakes the accept loop so it sees it should stop.
        let _ = std::net::TcpStream::connect(&self.addr);
    }
}

/// `gh auth token` prints the token file, or fails as `gh` does logged out.
fn write_gh(bin: &Path, token_file: &Path) {
    std::fs::create_dir_all(bin).unwrap();
    let script = format!(
        "#!/bin/sh\n\
         if [ \"$1 $2\" = \"auth token\" ]; then\n\
         \x20 cat '{}' 2>/dev/null && exit 0\n\
         \x20 echo 'no oauth token found for github.com' >&2\n\
         \x20 exit 1\n\
         fi\n\
         echo \"fake gh can't $*\" >&2\n\
         exit 1\n",
        token_file.display()
    );
    let gh = bin.join("gh");
    std::fs::write(&gh, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}
