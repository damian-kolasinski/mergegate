//! `mg hub start | stop | status | set | devices | allow | deny`.

mod devices;
pub(crate) mod probe;
pub(crate) mod report;
pub(crate) mod service;
mod set;
mod spawn;
mod start;
mod status;
mod stop;

use std::io;
use std::time::Duration;

use api::device::Access;
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
        /// Port to listen on, on 127.0.0.1 and, with listen tailnet, on the
        /// tailnet; 0 picks a free one
        #[arg(long, default_value_t = hub::DEFAULT_PORT)]
        port: u16,
    },
    /// Stop the hub (no-op if it is not running)
    Stop,
    /// Show whether the hub is running, and where
    Status,
    /// Change a hub setting: attachment-ttl, how long a merged or abandoned
    /// item keeps its attachments (2d by default; 0s drops them on close);
    /// listen, local or tailnet to let other machines on the tailnet connect
    Set {
        key: String,
        /// As in 2d, 12h, 30m or 0s; local or tailnet
        value: String,
    },
    /// List the tailnet's devices and which may reach the hub
    Devices,
    /// Let a tailnet device reach the hub
    Allow {
        /// Its MagicDNS name or address, as mg hub devices lists it
        device: String,
    },
    /// Keep a tailnet device out, even one of the hub owner's
    Deny {
        /// Its MagicDNS name or address, as mg hub devices lists it
        device: String,
    },
}

pub fn run(command: HubCommand) -> Result<Outcome, CliError> {
    let home = crate::home()?;
    match command {
        HubCommand::Start { foreground, port } => start::run(home, foreground, port),
        HubCommand::Stop => stop::run(&home),
        HubCommand::Status => status::run(&home),
        HubCommand::Set { key, value } => set::run(&home, key, value),
        HubCommand::Devices => devices::list(&home),
        HubCommand::Allow { device } => devices::decide(&home, device, Access::Allow),
        HubCommand::Deny { device } => devices::decide(&home, device, Access::Deny),
    }
}

fn io_error(context: &str, e: io::Error) -> CliError {
    CliError::new("io_error", format!("{context}: {e}"))
}
