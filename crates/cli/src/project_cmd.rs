//! `mg project add | show | set`.

use std::path::{Path, PathBuf};

use api::project::{AddProject, Backend, Project, Projects, SetProject};
use clap::{Subcommand, ValueEnum};

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
        /// Where approved work lands
        #[arg(long, value_enum, default_value_t = BackendArg::Local)]
        backend: BackendArg,
        /// Branch items merge into [default: main, master or trunk]
        #[arg(long)]
        target: Option<String>,
    },
    /// Show a project; without one, the project at the current directory, or
    /// every project when there is none
    Show { project: Option<String> },
    /// Change a project's backend, target or upstream
    Set {
        project: String,
        key: String,
        value: String,
        /// Apply even though open items keep the old backend or target
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
        } => add(&client, &path, name, backend.into(), target),
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
                "upstream" => absolute(Path::new(&value))?,
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

fn add(
    client: &Client,
    path: &Path,
    name: Option<String>,
    backend: Backend,
    target: Option<String>,
) -> Result<Outcome, CliError> {
    let request = AddProject {
        path: absolute(path)?,
        name,
        backend: Some(backend),
        target,
    };
    match client.post::<Project>("/api/projects", &request)? {
        Reply::Done(project) => Ok(Outcome::done(
            &project,
            format!("registered {}\n{}", project.name, describe(&project)),
        )),
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

/// The project whose repo contains the current directory.
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
                    p.backend.as_str(),
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
    format!(
        "  backend   {}\n  target    {}\n  upstream  {}",
        p.backend.as_str(),
        p.target,
        p.upstream
    )
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
