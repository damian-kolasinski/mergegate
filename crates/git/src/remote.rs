//! Reaching an upstream: the operator's repo by path, or a remote over HTTPS
//! with the credential the hub was given for it.

use std::ffi::OsStr;
use std::process::Command;

use mergegate_core::git::{Remote, Upstream};

/// What git is given to reach `upstream`.
pub(crate) fn location(upstream: &Upstream) -> &OsStr {
    match upstream {
        Upstream::Repo(root) => root.as_os_str(),
        Upstream::Remote(remote) => OsStr::new(&remote.url),
    }
}

/// How errors name `upstream`.
pub(crate) fn describe(upstream: &Upstream) -> String {
    match upstream {
        Upstream::Repo(root) => root.display().to_string(),
        Upstream::Remote(remote) => remote.url.clone(),
    }
}

/// Lets `cmd` sign in to `upstream`, if it's a remote. The header goes in
/// through the environment, never the arguments, so other users can't read
/// it in the process list, and is scoped to the remote's URL, so a submodule
/// or other remote git reaches on the way doesn't get it.
pub(crate) fn authorize(cmd: &mut Command, upstream: &Upstream) {
    if let Upstream::Remote(remote) = upstream {
        authorize_remote(cmd, remote);
    }
}

pub(crate) fn authorize_remote(cmd: &mut Command, remote: &Remote) {
    cmd.env("GIT_CONFIG_COUNT", "1")
        .env(
            "GIT_CONFIG_KEY_0",
            format!("http.{}.extraheader", remote.url),
        )
        .env("GIT_CONFIG_VALUE_0", remote.auth.expose());
}
