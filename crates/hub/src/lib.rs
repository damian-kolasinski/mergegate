//! The hub daemon: composition root of everything that runs inside it.

mod abandon;
mod attachments;
mod branches;
mod comments;
mod conflicts;
mod conversation;
mod demo_zip;
mod devices;
pub mod diagnostics;
mod diff;
mod doctor;
mod draft_replies;
mod event_log;
mod events;
mod external;
mod feedback;
mod git_http;
mod guard;
mod highlights;
mod home;
mod items;
mod judge;
mod languages;
mod lifecycle;
mod merge;
mod names;
mod presence;
mod priority;
mod projects;
mod remote_wakes;
mod replies;
mod reply;
mod resume;
mod review_time;
mod reviews;
mod rules;
mod server;
mod settings;
mod tailnet;
mod telemetry;
mod ui;
mod viewed;
mod wake;

use std::time::Duration;

pub use home::{Connection, HOME_ENV, Home, HomeError, HubInfo};
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
/// is written to the home's `hub.json`. `version` is the `mg` it reports
/// running, which only the binary that runs it knows in full.
pub fn run(home: Home, port: u16, version: &str) -> Result<(), HubError> {
    home.ensure().map_err(io("creating the data directory"))?;
    let _lock = home
        .acquire()
        .map_err(io("locking the data directory"))?
        .ok_or_else(|| HubError::Locked(home.root().display().to_string()))?;
    let _telemetry = telemetry::init(&home, version);
    let store = store_sqlite::Store::open(&home.db()).inspect_err(|error| {
        tracing::error!(
            event_code = "hub_start_failed",
            error_code = "store_open_failed",
            "{error}"
        );
    })?;
    let schema_version = store.schema_version()?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(io("starting the async runtime"))?;
    let served = runtime.block_on(server::serve(&home, port, version, schema_version, store));
    if let Err(error) = &served {
        tracing::error!(event_code = "hub_failed", "{error}");
    }
    // Dropping the runtime would wait out every blocking task, a judge call
    // too, which can run for minutes; serve set `stopping` for those that
    // can stop.
    runtime.shutdown_timeout(SHUTDOWN);
    served
}

/// How long a stopping hub waits for work off the runtime to finish.
const SHUTDOWN: Duration = Duration::from_secs(10);

/// The PATH a wake command runs with: this one behind `home`'s `bin`, which
/// holds only a link to the running `mg`, so `mg` in the command is this
/// one.
pub fn wake_path(home: &Home) -> Option<std::ffi::OsString> {
    wake::path_env(&home.wake_bin())
}
