//! `mg`: the mergegate CLI, and the hub it starts. Composition root of the CLI.

mod client;
mod doctor_cmd;
mod feedback_cmd;
mod hub_cmd;
mod inbox_cmd;
mod init_cmd;
pub mod output;
mod project_cmd;
mod repo;
mod submit_cmd;
mod wait_cmd;

use std::ffi::OsString;
use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
use hub::Home;

use output::{CliError, EXIT_CODES_HELP, Outcome, exit};

/// Set to `1` to get `--json` output from every command.
pub const JSON_ENV: &str = "MG_JSON";

#[derive(Parser)]
#[command(
    name = "mg",
    version,
    about = "mergegate: local code review hub for coding agents",
    after_help = EXIT_CODES_HELP,
    arg_required_else_help = true,
    disable_help_subcommand = true
)]
struct Cli {
    /// Print one JSON object on stdout (also: MG_JSON=1)
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run and inspect the hub
    #[command(subcommand)]
    Hub(hub_cmd::HubCommand),
    /// Register repos as projects and change their settings
    #[command(subcommand)]
    Project(project_cmd::ProjectCommand),
    /// In a clone: add the `mergegate` remote, pointing at its project
    Init {
        /// Project to push to [default: the one registered for this repo]
        #[arg(long)]
        project: Option<String>,
    },
    /// Push the current branch to the hub and submit it for review; again
    /// from the same branch, add a revision
    Submit {
        /// Item title [default: the first commit's subject]
        #[arg(long)]
        title: Option<String>,
    },
    /// Wait until the item has feedback for its agent, print it and mark it
    /// delivered
    Wait {
        /// The item, as in app#42
        item: String,
        /// Which events end the wait
        #[arg(long = "for", value_enum, default_value = "any")]
        wake: wait_cmd::Wake,
        /// Seconds to wait before exiting with 75; 0 waits until there is
        /// feedback
        #[arg(long, default_value_t = 540)]
        timeout: u64,
        /// Return the events after this cursor, even delivered ones
        #[arg(long)]
        since: Option<u64>,
    },
    /// Print the item's new feedback and mark it delivered, without waiting
    Feedback {
        /// The item, as in app#42
        item: String,
        /// Return the events after this cursor, even delivered ones
        #[arg(long)]
        since: Option<u64>,
    },
    /// Open items: those waiting for review, with their agents, merging
    Inbox,
    /// Check that the hub, git and every project are set up to work
    Doctor,
}

pub fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let json_env = std::env::var_os(JSON_ENV).is_some_and(|v| v == "1");
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => return usage(e, json_env || args.iter().any(|a| a == "--json")),
    };
    let json = cli.json || json_env;
    output::emit(json, dispatch(cli.command))
}

fn dispatch(command: Command) -> Result<Outcome, CliError> {
    match command {
        Command::Hub(command) => hub_cmd::run(command),
        Command::Project(command) => project_cmd::run(command),
        Command::Init { project } => init_cmd::run(project),
        Command::Submit { title } => submit_cmd::run(title),
        Command::Wait {
            item,
            wake,
            timeout,
            since,
        } => wait_cmd::run(item, wake, timeout, since),
        Command::Feedback { item, since } => feedback_cmd::run(item, since),
        Command::Inbox => inbox_cmd::run(),
        Command::Doctor => doctor_cmd::run(),
    }
}

fn home() -> Result<Home, CliError> {
    Home::from_env().map_err(|e| {
        CliError::new("invalid_home", e.to_string())
            .hint(format!("set {} to an absolute path", hub::HOME_ENV))
    })
}

/// clap exits with 2 on usage errors, which the contract reserves for
/// `needs_decision`; usage errors exit with 64 instead.
fn usage(e: clap::Error, json: bool) -> ExitCode {
    match e.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            let _ = e.print();
            ExitCode::from(exit::OK)
        }
        _ if json => {
            let rendered = e.render().to_string();
            let message = match e.kind() {
                // clap renders help here, not an error line.
                ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => "a subcommand is required",
                _ => rendered
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .trim_start_matches("error: "),
            };
            let err = CliError::new("usage", message)
                .hint("mg --help")
                .exit(exit::USAGE);
            output::emit(true, Err(err))
        }
        _ => {
            let _ = e.print();
            ExitCode::from(exit::USAGE)
        }
    }
}
