//! `mg init`: points a clone's `mergegate` remote at its project on the hub.

use api::project::Project;
use serde::Serialize;

use crate::client::Client;
use crate::output::{CliError, Outcome};
use crate::project_cmd::{project_here, route};
use crate::repo::{self, REMOTE};

#[derive(Serialize)]
struct Initialized {
    project: String,
    remote: &'static str,
    url: String,
}

pub fn run(project: Option<String>) -> Result<Outcome, CliError> {
    repo::require()?;
    let client = Client::connect(&crate::home()?)?;
    let project: Project = match project {
        Some(name) => client.get(&route(&name)?)?,
        None => project_here(&client).map_err(|e| match e.body.code.as_str() {
            "no_project" => e.hint("mg init --project NAME"),
            _ => e,
        })?,
    };
    let url = client.remote(&project.name);
    match repo::read(&["remote", "get-url", REMOTE])? {
        Some(current) if current == url => {}
        Some(_) => repo::run(
            "pointing the mergegate remote at the hub",
            &["remote", "set-url", REMOTE, &url],
        )?,
        None => repo::run(
            "adding the mergegate remote",
            &["remote", "add", REMOTE, &url],
        )?,
    }
    let text = format!("remote {REMOTE} -> {url} (project {})", project.name);
    Ok(Outcome::done(
        &Initialized {
            project: project.name,
            remote: REMOTE,
            url,
        },
        text,
    ))
}
