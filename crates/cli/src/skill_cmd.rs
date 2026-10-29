//! `mg skill`: sets coding agents up to work through mergegate. `setup`
//! puts a stub where each agent loads skills, pointing at `mg skill show`,
//! and a session hook in those that run one; `status` and `remove` report
//! and undo it. `install` writes the whole skill for agents setup doesn't
//! know.

mod agents;
mod install;
mod session_start;
mod show;

use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};

use crate::output::{CliError, Outcome};

#[derive(Subcommand)]
pub enum SkillCommand {
    /// Install the skill and session hook into the agents on this machine,
    /// or those named with --agent; again, update what an older mg installed
    Setup {
        /// Only AGENT; repeat for more [default: every agent found]
        #[arg(long = "agent", value_enum, value_name = "AGENT")]
        agents: Vec<AgentArg>,
        /// Set up the current repository instead of your account
        #[arg(long)]
        project: bool,
        /// Report what would change without changing anything
        #[arg(long)]
        dry_run: bool,
        /// Replace a skill named mergegate that mg didn't write
        #[arg(long)]
        force: bool,
    },
    /// Report whether each agent has the skill and session hook
    Status {
        /// Report on the current repository instead of your account
        #[arg(long)]
        project: bool,
    },
    /// Remove the skill and session hook from every agent, or those named
    /// with --agent, leaving everything mg didn't install
    Remove {
        /// Only AGENT; repeat for more [default: every agent mg knows]
        #[arg(long = "agent", value_enum, value_name = "AGENT")]
        agents: Vec<AgentArg>,
        /// Remove from the current repository instead of your account
        #[arg(long)]
        project: bool,
        /// Report what would change without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Print the guide the skill points agents at, or the reference NAME
    Show {
        /// One of the references the guide names, as in waiting
        name: Option<String>,
    },
    /// Write the whole skill folder and print where it is, for an agent
    /// setup doesn't know
    Install {
        /// Write it to PATH/mergegate [default: $MERGEGATE_HOME/skill]
        #[arg(long, value_name = "PATH")]
        dir: Option<PathBuf>,
    },
    /// What the session hook tells a new session of AGENT
    #[command(hide = true)]
    SessionStart {
        #[arg(long, value_enum)]
        agent: AgentArg,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AgentArg {
    Claude,
    Codex,
    #[value(name = "opencode")]
    OpenCode,
}

impl From<AgentArg> for skill::Agent {
    fn from(arg: AgentArg) -> Self {
        match arg {
            AgentArg::Claude => skill::Agent::Claude,
            AgentArg::Codex => skill::Agent::Codex,
            AgentArg::OpenCode => skill::Agent::OpenCode,
        }
    }
}

pub fn run(command: SkillCommand) -> Result<Outcome, CliError> {
    match command {
        SkillCommand::Setup {
            agents: named,
            project,
            dry_run,
            force,
        } => agents::setup(&named, project, dry_run, force),
        SkillCommand::Status { project } => agents::status(project),
        SkillCommand::Remove {
            agents: named,
            project,
            dry_run,
        } => agents::remove(&named, project, dry_run),
        SkillCommand::Show { name } => show::run(name),
        SkillCommand::Install { dir } => install::run(dir),
        SkillCommand::SessionStart { agent } => Ok(session_start::run(agent.into())),
    }
}
