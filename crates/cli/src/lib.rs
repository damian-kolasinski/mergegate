//! `mg`: the mergegate CLI, and the hub it starts. Composition root of the CLI.

mod hub_cmd;
pub mod output;

use std::ffi::OsString;
use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

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
    }
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
