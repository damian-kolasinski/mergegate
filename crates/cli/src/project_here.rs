//! The project the clone at the current directory works on: the one whose
//! repo it is, or else the one its remotes point at, as a clone of the
//! project from GitHub, from the hub, or from its repo on the hub's machine.

use std::path::Path;

use api::project::{Project, Projects};
use mergegate_core::github::GithubRepo;

use crate::client::Client;
use crate::output::CliError;
use crate::project_cmd::absolute;
use crate::repo::{self, REMOTE};

pub(crate) fn project_here(client: &Client) -> Result<Project, CliError> {
    // A connected hub would look the path up on its own machine, where it
    // names nothing, or something else.
    let unregistered = match client.connected() {
        true => {
            repo::require()?;
            None
        }
        false => {
            let cwd = std::env::current_dir().map_err(|e| {
                CliError::new("io_error", format!("reading the current directory: {e}"))
            })?;
            let path = absolute(&cwd)?;
            match client.get_query("/api/project-at", &[("path", &path)]) {
                Err(e) if e.body.code == "no_project" => Some(e),
                found => return found,
            }
        }
    };
    let local = !client.connected();
    let remotes = remotes(local)?;
    let projects: Projects = client.get("/api/projects")?;
    let matched = matching(&projects.projects, &remotes, local, |name| {
        client.remote(name)
    });
    match matched.as_slice() {
        [project] => Ok((*project).clone()),
        [] => Err(unregistered.unwrap_or_else(|| {
            CliError::new(
                "no_project",
                "no project on the hub matches this clone's remotes",
            )
        })),
        several => {
            let names: Vec<&str> = several.iter().map(|p| p.name.as_str()).collect();
            Err(CliError::new(
                "ambiguous_project",
                format!(
                    "this clone's remotes point at several projects: {}",
                    names.join(", ")
                ),
            ))
        }
    }
}

/// The clone's remotes, as name and URL. On the hub's machine a URL that is
/// a path is resolved, to compare with the projects' repos.
fn remotes(local: bool) -> Result<Vec<(String, String)>, CliError> {
    // Fails, as when there are no remotes, with nothing to read.
    let listed = repo::read(&["config", "--get-regexp", r"^remote\..*\.url$"])?;
    Ok(listed
        .iter()
        .flat_map(|listed| listed.lines())
        .filter_map(|line| {
            let (key, url) = line.split_once(' ')?;
            let name = key.strip_prefix("remote.")?.strip_suffix(".url")?;
            let path = url.strip_prefix("file://").unwrap_or(url);
            let url = match local && Path::new(path).is_absolute() {
                true => absolute(Path::new(path)).unwrap_or_else(|_| url.to_owned()),
                false => url.to_owned(),
            };
            Some((name.to_owned(), url))
        })
        .collect())
}

/// The projects one of `remotes` points at: the project's remote on the hub
/// (`hub_remote` of its name), its GitHub repository, or, when `local` to
/// the hub, its repo. The `mergegate` remote settles it when it names one:
/// `mg init` set it, and projects may share a GitHub repository.
fn matching<'a>(
    projects: &'a [Project],
    remotes: &[(String, String)],
    local: bool,
    hub_remote: impl Fn(&str) -> String,
) -> Vec<&'a Project> {
    let ours = remotes
        .iter()
        .find(|(name, _)| name == REMOTE)
        .map(|(_, url)| url.trim_end_matches('/'));
    if let Some(project) = ours.and_then(|url| projects.iter().find(|p| hub_remote(&p.name) == url))
    {
        return vec![project];
    }
    let github = |url: &str| GithubRepo::from_url(url).map(|r| r.to_string().to_lowercase());
    projects
        .iter()
        .filter(|project| {
            let on_hub = hub_remote(&project.name);
            let on_github = project.github.as_deref().map(str::to_lowercase);
            remotes.iter().any(|(_, url)| {
                let url = url.trim_end_matches('/');
                url == on_hub
                    || (local && url == project.upstream)
                    || (on_github.is_some() && github(url) == on_github)
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "project_here_tests.rs"]
mod tests;
