use std::time::{Duration, Instant};

use hub::{Home, HubError, StoreError};

use super::probe::{Running, probe};
use super::report::{HubStatus, unresponsive};
use super::spawn::Spawned;
use super::{POLL, io_error};
use crate::output::{CliError, Outcome, exit};

const TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn run(home: Home, foreground: bool, port: u16) -> Result<Outcome, CliError> {
    if foreground {
        return run_here(home, port);
    }
    home.ensure()
        .map_err(|e| io_error("creating the data directory", e))?;
    if let Some(running) = probe(&home) {
        if !running.compatible() {
            return Err(incompatible(&running));
        }
        return Ok(outcome(&home, &running, false));
    }

    let mut spawned: Option<Spawned> = None;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        // A locked home has a hub starting, stopping or hung: never race it;
        // start ours once the lock is free.
        if spawned.is_none() && !home.is_locked() {
            spawned = Some(Spawned::new(&home, port)?);
        }
        if let Some(running) = probe(&home) {
            let ours = spawned
                .as_ref()
                .is_some_and(|s| s.pid() == running.health.pid);
            if !ours && let Some(s) = spawned.as_mut() {
                // Else ours, still waiting for the lock, would take over the
                // home once the winner stops.
                s.abandon();
            }
            return Ok(outcome(&home, &running, ours));
        }
        if let Some(s) = spawned.as_mut()
            && let Some(err) = s.failure(&home)
        {
            if err.body.code != "hub_locked" {
                return Err(err);
            }
            // A concurrent start won the race; wait for its hub.
            spawned = None;
        }
        if Instant::now() > deadline {
            if spawned.is_none() && home.is_locked() {
                return Err(unresponsive(&home));
            }
            return Err(CliError::new(
                "hub_start_timeout",
                format!("the hub did not answer within {TIMEOUT:?}; it may still be starting"),
            )
            .hint(format!("mg hub status; see {}", home.log().display()))
            .exit(exit::TIMEOUT));
        }
        std::thread::sleep(POLL);
    }
}

fn outcome(home: &Home, running: &Running, started: bool) -> Outcome {
    let status = HubStatus {
        started: Some(started),
        ..running.status(home)
    };
    let verb = if started {
        "started"
    } else {
        "already running"
    };
    Outcome::done(&status, format!("hub {verb} at {}", running.describe()))
}

fn run_here(home: Home, port: u16) -> Result<Outcome, CliError> {
    hub::init_logging();
    let status = HubStatus::stopped(&home, true);
    hub::run(home, port).map_err(hub_error)?;
    Ok(Outcome::done(&status, "hub stopped"))
}

fn incompatible(running: &Running) -> CliError {
    CliError::new(
        "hub_incompatible",
        format!(
            "the running hub speaks API v{}, this mg speaks v{}",
            running.health.api_version,
            api::API_VERSION
        ),
    )
    .hint("mg hub stop && mg hub start")
}

fn hub_error(e: HubError) -> CliError {
    let message = e.to_string();
    match e {
        HubError::Locked(_) => CliError::new("hub_locked", message).hint("mg hub status"),
        HubError::PortInUse(_) => {
            CliError::new("port_in_use", message).hint("mg hub start --port N")
        }
        HubError::Store(StoreError::SchemaTooNew { .. }) => {
            CliError::new("schema_too_new", message).hint(
                "install the mg version that last ran this hub, or restore a hub.db.bak-* backup",
            )
        }
        HubError::Store(_) => CliError::new("store_error", message),
        HubError::Io { .. } => CliError::new("io_error", message),
    }
}
