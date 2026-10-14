//! The operator's GitHub token, as `gh auth token` gives it.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;

use mergegate_core::git::Secret;
use mergegate_core::github::GithubError;

pub(crate) struct Token {
    gh: PathBuf,
    cached: Mutex<Option<Secret>>,
}

impl Token {
    pub fn new(gh: PathBuf) -> Self {
        Self {
            gh,
            cached: Mutex::new(None),
        }
    }

    pub fn get(&self) -> Result<Secret, GithubError> {
        let mut cached = self.cached.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(token) = &*cached {
            return Ok(token.clone());
        }
        let token = read(&self.gh)?;
        *cached = Some(token.clone());
        Ok(token)
    }

    /// After GitHub turned the token down: `gh` may have a new one by now.
    pub fn forget(&self) {
        *self.cached.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// `gh` runs in the hub's environment, which is the operator's, so it finds
/// their login wherever `gh` keeps it: the keychain, its config, `GH_TOKEN`.
fn read(gh: &PathBuf) -> Result<Secret, GithubError> {
    let output = Command::new(gh)
        .args(["auth", "token", "--hostname", "github.com"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            GithubError::NotLoggedIn(format!(
                "can't run gh, the GitHub CLI the hub signs in with: {e}"
            ))
        })?;
    let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || token.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let why = match stderr.trim() {
            "" => output.status.to_string(),
            stderr => stderr.to_owned(),
        };
        return Err(GithubError::NotLoggedIn(format!(
            "gh auth token failed: {why}"
        )));
    }
    Ok(Secret::new(token))
}
