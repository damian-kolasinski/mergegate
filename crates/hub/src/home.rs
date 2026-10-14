//! The data directory (`MERGEGATE_HOME`, default `~/.mergegate`) shared by the
//! hub and the CLI, and the files in it.

use std::fs::File;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub const HOME_ENV: &str = "MERGEGATE_HOME";

/// How long a starting hub waits for a briefly held lock before giving up.
const LOCK_PATIENCE: Duration = Duration::from_millis(500);

#[derive(Debug, thiserror::Error)]
pub enum HomeError {
    #[error("{HOME_ENV} must be an absolute path, got {0:?}")]
    Relative(PathBuf),
    #[error("neither {HOME_ENV} nor HOME is set")]
    Unset,
}

/// Written by a running hub so the CLI can find it; removed when it stops.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubInfo {
    pub addr: SocketAddr,
    pub pid: u32,
}

#[derive(Debug, Clone)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    pub fn from_env() -> Result<Self, HomeError> {
        match std::env::var_os(HOME_ENV).filter(|v| !v.is_empty()) {
            Some(root) => {
                let root = PathBuf::from(root);
                if root.is_relative() {
                    return Err(HomeError::Relative(root));
                }
                Ok(Self { root })
            }
            None => {
                let home = std::env::var_os("HOME")
                    .filter(|v| !v.is_empty())
                    .ok_or(HomeError::Unset)?;
                Ok(Self {
                    root: Path::new(&home).join(".mergegate"),
                })
            }
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn db(&self) -> PathBuf {
        self.root.join("hub.db")
    }

    pub fn log(&self) -> PathBuf {
        self.root.join("hub.log")
    }

    /// Per-project data, such as staging repos, one folder per project.
    pub fn projects(&self) -> PathBuf {
        self.root.join("projects")
    }

    /// The stderr of each item's newest wake command, one file per item.
    pub fn wake_logs(&self) -> PathBuf {
        self.root.join("wakes")
    }

    /// What the hub's git commands see as `HOME`, so the settings of the user
    /// running the hub never apply to them.
    pub fn git_home(&self) -> PathBuf {
        self.root.join("git-home")
    }

    fn info_path(&self) -> PathBuf {
        self.root.join("hub.json")
    }

    fn lock_path(&self) -> PathBuf {
        self.root.join("hub.lock")
    }

    pub fn ensure(&self) -> io::Result<()> {
        std::fs::create_dir_all(&self.root)
    }

    /// The running hub's address, if `hub.json` exists and parses. The hub may
    /// still be gone; callers confirm with a health check.
    pub fn read_info(&self) -> Option<HubInfo> {
        let bytes = std::fs::read(self.info_path()).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub(crate) fn write_info(&self, info: &HubInfo) -> io::Result<()> {
        let tmp = self.root.join("hub.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(info)?)?;
        std::fs::rename(tmp, self.info_path())
    }

    pub(crate) fn remove_info(&self, pid: u32) {
        if self.read_info().is_some_and(|info| info.pid == pid) {
            let _ = std::fs::remove_file(self.info_path());
        }
    }

    /// Takes the lock that allows one hub per home, retrying briefly: a CLI
    /// checking [`Home::is_locked`] holds it for an instant, and that must not
    /// make a starting hub give up. It is held until the returned file is
    /// dropped or the process exits, crashes included.
    pub(crate) fn acquire(&self) -> io::Result<Option<File>> {
        let deadline = Instant::now() + LOCK_PATIENCE;
        loop {
            match self.lock()? {
                Some(file) => return Ok(Some(file)),
                None if Instant::now() > deadline => return Ok(None),
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    }

    fn lock(&self) -> io::Result<Option<File>> {
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.lock_path())?;
        match file.try_lock() {
            Ok(()) => Ok(Some(file)),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }

    /// Whether a hub process holds this home's lock, responsive or not.
    pub fn is_locked(&self) -> bool {
        // A missing lock file means no hub has ever started here.
        if !self.lock_path().exists() {
            return false;
        }
        matches!(self.lock(), Ok(None))
    }
}
