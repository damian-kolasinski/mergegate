//! The hub daemon.

mod guard;
mod home;
mod lifecycle;
mod server;

use std::io::IsTerminal;

pub use home::{HOME_ENV, Home, HomeError, HubInfo};
pub use store_sqlite::{SCHEMA_VERSION, StoreError};

pub const DEFAULT_PORT: u16 = 7800;

#[derive(Debug, thiserror::Error)]
pub enum HubError {
    #[error("another hub is already running with data in {0}")]
    Locked(String),
    #[error("port {0} is already in use")]
    PortInUse(u16),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        source: std::io::Error,
    },
}

fn io(context: &'static str) -> impl FnOnce(std::io::Error) -> HubError {
    move |source| HubError::Io { context, source }
}

/// Runs the hub in this process until it is asked to stop, over HTTP or by
/// SIGINT/SIGTERM. `port` 0 picks a free port; the address the hub listens on
/// is written to the home's `hub.json`.
pub fn run(home: Home, port: u16) -> Result<(), HubError> {
    home.ensure().map_err(io("creating the data directory"))?;
    let _lock = home
        .acquire()
        .map_err(io("locking the data directory"))?
        .ok_or_else(|| HubError::Locked(home.root().display().to_string()))?;
    let store = store_sqlite::Store::open(&home.db())?;
    let schema_version = store.schema_version()?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(io("starting the async runtime"))?;
    runtime.block_on(server::serve(&home, port, schema_version))
}

/// Logs to stderr, which a backgrounded hub sends to its `hub.log`.
pub fn init_logging() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .with_target(false)
        .init();
}
