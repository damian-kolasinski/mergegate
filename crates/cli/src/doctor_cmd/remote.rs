//! Whether the clone at the current directory pushes to its project on the
//! hub, as `mg init` sets it up.

use api::doctor::{Check, CheckStatus};

use crate::client::Client;
use crate::hub_cmd::probe::Running;
use crate::output::CliError;
use crate::project_cmd::project_here;
use crate::repo::{self, REMOTE};

/// None outside a git repo, and in one that is neither a project's nor set
/// up for one.
pub(super) fn check(running: &Running, client: &Client) -> Result<Option<Check>, CliError> {
    if repo::read(&["rev-parse", "--git-dir"])?.is_none() {
        return Ok(None);
    }
    let remote = repo::read(&["remote", "get-url", REMOTE])?;
    let project = match project_here(client) {
        Ok(project) => Some(project.name),
        Err(e) if matches!(e.body.code.as_str(), "no_project" | "not_a_repo") => None,
        // The checks already made matter more than this one.
        Err(e) => {
            return Ok(Some(Check {
                name: "remote".into(),
                status: CheckStatus::Warn,
                message: format!("can't tell which project this clone is: {}", e.body.message),
                fix: e.body.hint,
            }));
        }
    };
    let expected = project.as_deref().map(|name| (name, client.remote(name)));
    let hub = format!("{}/git/", running.url());
    Ok(judge(
        expected.as_ref().map(|(name, url)| (*name, url.as_str())),
        remote.as_deref(),
        &hub,
    ))
}

/// `project` is the one at the current directory with the remote URL it
/// expects; `hub` is the prefix of every project's on this hub.
fn judge(project: Option<(&str, &str)>, remote: Option<&str>, hub: &str) -> Option<Check> {
    let check = |status, message: String, fix: Option<String>| {
        Some(Check {
            name: "remote".into(),
            status,
            message,
            fix,
        })
    };
    match (project, remote) {
        (Some((name, expected)), Some(url)) if url == expected => check(
            CheckStatus::Ok,
            format!("{REMOTE} pushes to project {name}"),
            None,
        ),
        (Some((name, _)), Some(url)) => check(
            CheckStatus::Warn,
            format!("{REMOTE} points at {url}, not project {name}"),
            Some("mg init".into()),
        ),
        (Some((name, _)), None) => check(
            CheckStatus::Warn,
            format!("this clone of project {name} has no {REMOTE} remote"),
            Some("mg init".into()),
        ),
        // A clone the hub can't place, e.g. registered under another path.
        (None, Some(url)) if url.starts_with(hub) => {
            check(CheckStatus::Ok, format!("{REMOTE} pushes to {url}"), None)
        }
        (None, Some(url)) => check(
            CheckStatus::Warn,
            format!("{REMOTE} points at {url}, not this hub"),
            Some("mg init --project NAME".into()),
        ),
        (None, None) => None,
    }
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
