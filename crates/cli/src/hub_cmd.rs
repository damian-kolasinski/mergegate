//! `mg hub start | stop | status`.

pub(crate) mod probe;
mod report;
mod spawn;
mod start;
mod status;
mod stop;

use std::io;
use std::time::Duration;

use clap::Subcommand;

use crate::output::{CliError, Outcome};

const POLL: Duration = Duration::from_millis(10);

#[derive(Subcommand)]
pub enum HubCommand {
    /// Start the hub in the background (no-op if it is running)
    Start {
        /// Run in this process instead of the background
        #[arg(long)]
        foreground: bool,
        /// Port to listen on, 127.0.0.1 only; 0 picks a free one
        #[arg(long, default_value_t = hub::DEFAULT_PORT)]
        port: u16,
    },
    /// Stop the hub (no-op if it is not running)
    Stop,
    /// Show whether the hub is running, and where
    Status,
}

pub fn run(command: HubCommand) -> Result<Outcome, CliError> {
    let home = crate::home()?;
    match command {
        HubCommand::Start { foreground, port } => start::run(home, foreground, port),
        HubCommand::Stop => stop::run(&home),
        HubCommand::Status => status::run(&home),
    }
}

fn io_error(context: &str, e: io::Error) -> CliError {
    CliError::new("io_error", format!("{context}: {e}"))
}
