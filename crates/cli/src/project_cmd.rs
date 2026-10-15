//! `mg project add | show | set`.

use std::path::{Path, PathBuf};

use api::project::{AddProject, Added, Backend, Project, Projects, SetProject};
use clap::{Subcommand, ValueEnum};
use mergegate_core::project::NONE;

use crate::client::{Client, Reply, segment};
use crate::output::{CliError, Outcome};

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Register the git repo containing PATH as a project
    Add {
        /// Any path inside the repo; a linked worktree registers its main repo
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Name used in item IDs [default: the repo's folder name]
        #[arg(long)]
        name: Option<String>,
        /// Where approved work lands [default: local, unless the repo has
        /// a GitHub remote, when the operator is asked]
        #[arg(long, value_enum)]
        backend: Option<BackendArg>,
        /// Branch items merge into [default: main, master or trunk]
        #[arg(long)]
        target: Option<String>,
        /// The GitHub repository items publish to, as OWNER/REPO
        /// [default: the one the remotes point at]; implies --backend github
        #[arg(long, value_name = "OWNER/REPO")]
        github: Option<String>,
        /// The language packs to install, comma-separated, or none
        /// [default: asked, when the repo uses packs that aren't installed]
        #[arg(long, value_name = "LIST|none")]
        languages: Option<String>,
    },
    /// Show a project; without one, the project at the current directory, or
    /// every project when there is none
    Show { project: Option<String> },
    /// Change a project's backend, target, upstream, GitHub repository, or
    /// bots: the GitHub logins, comma-separated, whose reviews to wait for
    Set {
        project: String,
        key: String,
        value: String,
        /// Apply even though open items keep the old backend, repository or
        /// target
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum BackendArg {
    Github,
    Local,
}

impl From<BackendArg> for Backend {
    fn from(backend: BackendArg) -> Self {
        match backend {
            BackendArg::Github => Self::Github,
            BackendArg::Local => Self::Local,
        }
    }
}

pub fn run(command: ProjectCommand) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    match command {
        ProjectCommand::Add {
            path,
            name,
            backend,
            target,
            github,
            languages,
        } => {
            let request = AddProject {
                path: local_path(&client, &path)?,
                name,
                backend: backend.map(Into::into),
                target,
                github,
                languages: languages.as_deref().map(language_list),
            };
            add(&client, &request)
        }
        ProjectCommand::Show {
            project: Some(name),
        } => {
            let project = client.get(&route(&name)?)?;
            Ok(one(&project))
        }
        ProjectCommand::Show { project: None } => show_here(&client),
        ProjectCommand::Set {
            project,
            key,
            value,
            confirm,
        } => {
            let value = match key.as_str() {
                "upstream" => local_path(&client, Path::new(&value))?,
                _ => value,
            };
            let request = SetProject {
                key,
                value,
                confirm,
            };
            match client.patch::<Project>(&route(&project)?, &request)? {
                Reply::Done(project) => Ok(one(&project)),
                Reply::Decide(decision) => Ok(Outcome::NeedsDecision(decision)),
            }
        }
    }
}

/// The route of one project. An empty name would address no route at all.
pub(crate) fn route(name: &str) -> Result<String, CliError> {
    if name.is_empty() {
        return Err(
            CliError::new("project_not_found", "the project name is empty").hint("mg project show"),
        );
    }
    Ok(format!("/api/projects/{}", segment(name)))
}

/// `--languages`: names, comma-separated, or `none`.
fn language_list(value: &str) -> Vec<String> {
    if value.trim() == NONE {
        return Vec::new();
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(Into::into)
        .collect()
}

fn add(client: &Client, request: &AddProject) -> Result<Outcome, CliError> {
    match client.post::<Added>("/api/projects", request)? {
        Reply::Done(added) => {
            let project = &added.project;
            let mut text = format!("registered {}\n{}", project.name, describe(project));
            if !added.languages.is_empty() {
                let names: Vec<&str> = added.languages.iter().map(|p| p.name.as_str()).collect();
                text.push_str(&format!("\n  languages {}", names.join(", ")));
            }
            for warning in &added.warnings {
                text.push_str(&format!("\nwarning: {warning}"));
            }
            Ok(Outcome::done(&added, text))
        }
        Reply::Decide(decision) => Ok(Outcome::NeedsDecision(decision)),
    }
}

fn show_here(client: &Client) -> Result<Outcome, CliError> {
    match project_here(client) {
        Ok(project) => Ok(one(&project)),
        Err(e) if matches!(e.body.code.as_str(), "no_project" | "not_a_repo") => all(client),
        Err(e) => Err(e),
    }
}

/// `project`, or the one at the current directory.
pub(crate) fn named(client: &Client, project: Option<String>) -> Result<String, CliError> {
    match project {
        Some(project) => Ok(project),
        None => Ok(project_here(client)?.name),
    }
}

pub(crate) fn project_here(client: &Client) -> Result<Project, CliError> {
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::new("io_error", format!("reading the current directory: {e}")))?;
    let path = absolute(&cwd)?;
    client.get_query("/api/project-at", &[("path", &path)])
}

fn all(client: &Client) -> Result<Outcome, CliError> {
    let list: Projects = client.get("/api/projects")?;
    let text = if list.projects.is_empty() {
        "no projects; register one with mg project add PATH".to_owned()
    } else {
        let width = list
            .projects
            .iter()
            .map(|p| p.name.len())
            .max()
            .unwrap_or(0);
        list.projects
            .iter()
            .map(|p| {
                format!(
                    "{:width$}  {}  {}  {}",
                    p.name,
                    landing(p),
                    p.target,
                    p.upstream
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(Outcome::done(&list, text))
}

fn one(p: &Project) -> Outcome {
    Outcome::done(p, format!("{}\n{}", p.name, describe(p)))
}

fn describe(p: &Project) -> String {
    let mut text = format!(
        "  backend   {}\n  target    {}\n  upstream  {}",
        p.backend.as_str(),
        p.target,
        p.upstream
    );
    if let Some(github) = &p.github {
        text.push_str(&format!("\n  github    {github}"));
    }
    if !p.bots.is_empty() {
        text.push_str(&format!("\n  bots      {}", p.bots.join(", ")));
    }
    text
}

/// The backend, with the repository a `github` one publishes to.
fn landing(p: &Project) -> String {
    match (p.backend, &p.github) {
        (Backend::Github, Some(github)) => format!("github {github}"),
        (backend, _) => backend.as_str().to_owned(),
    }
}

/// A path on this machine for the hub to read, which a hub on another
/// machine can't.
pub(crate) fn local_path(client: &Client, path: &Path) -> Result<String, CliError> {
    if client.connected() {
        return Err(CliError::new(
            "path_remote",
            format!(
                "{} is on this machine, and the hub is on another",
                path.display()
            ),
        )
        .hint("run it on the hub's machine"));
    }
    absolute(path)
}

/// The hub may run with another working directory, so paths go to it
/// absolute and with symlinks resolved.
fn absolute(path: &Path) -> Result<String, CliError> {
    let resolved = std::fs::canonicalize(path)
        .map_err(|e| CliError::new("invalid_path", format!("{}: {e}", path.display())))?;
    resolved.into_os_string().into_string().map_err(|path| {
        CliError::new(
            "invalid_path",
            format!("{} is not valid UTF-8", Path::new(&path).display()),
        )
    })
}
