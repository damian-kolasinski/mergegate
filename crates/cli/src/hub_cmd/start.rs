use std::time::{Duration, Instant};

use hub::{Home, HubError, StoreError};

use super::probe::{Running, probe};
use super::report::{HubStatus, diagnostics_hint, unresponsive};
use super::service::{self, Loaded, Role};
use super::spawn::Spawned;
use super::ui_link;
use super::{POLL, io_error};
use crate::output::{CliError, Outcome, exit};

const TIMEOUT: Duration = Duration::from_secs(10);

/// How this start set its hub going.
enum Launched {
    Spawned(Spawned),
    Service(Loaded),
}

pub(super) fn run(home: Home, foreground: bool, port: u16) -> Result<Outcome, CliError> {
    if foreground {
        return run_here(home, port);
    }
    home.ensure()
        .map_err(|e| io_error("creating the data directory", e))?;
    let mut warnings = Vec::new();
    let mut use_service = service::applies();
    if let Some(running) = probe(&home) {
        if !running.compatible() {
            return Err(incompatible(&running));
        }
        if use_service && service::installed(Role::Hub).is_none() {
            warnings.push(
                "the hub runs without a service, so it won't start at login; \
                 mg hub stop && mg hub start installs one"
                    .to_owned(),
            );
        }
        return Ok(outcome(&home, &running, false, warnings));
    }

    let mut launched: Option<Launched> = None;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        // A locked home has a hub starting, stopping or hung: never race it;
        // start ours once the lock is free.
        if launched.is_none() && !home.is_locked() {
            if use_service {
                let port = port.to_string();
                let start = ["--json", "hub", "start", "--foreground", "--port", &port];
                match service::load(&home, Role::Hub, &start) {
                    Ok(loaded) => launched = Some(Launched::Service(loaded)),
                    Err(why) => {
                        use_service = false;
                        warnings.push(format!(
                            "{why}; started the hub without a service, so it won't start at login"
                        ));
                    }
                }
            }
            if !use_service {
                launched = Some(Launched::Spawned(Spawned::new(&home, port)?));
            }
        }
        if let Some(running) = probe(&home) {
            let ours = match &launched {
                Some(Launched::Spawned(s)) => s.pid() == running.health.pid,
                // The manager runs one hub per unit; whichever answers is it.
                Some(Launched::Service(_)) => true,
                None => false,
            };
            if !ours && let Some(Launched::Spawned(s)) = launched.as_mut() {
                // Else ours, still waiting for the lock, would take over the
                // home once the winner stops.
                s.abandon();
            }
            return Ok(outcome(&home, &running, ours, warnings));
        }
        match launched.as_mut() {
            Some(Launched::Spawned(s)) => {
                if let Some(err) = s.failure(&home) {
                    if err.body.code != "hub_locked" {
                        return Err(err);
                    }
                    // A concurrent start won the race; wait for its hub.
                    launched = None;
                }
            }
            Some(Launched::Service(loaded)) => {
                if let Some(err) = loaded.failure(&home) {
                    loaded.stop_retrying();
                    return Err(err);
                }
            }
            None => {}
        }
        if Instant::now() > deadline {
            if launched.is_none() && home.is_locked() {
                return Err(unresponsive(&home));
            }
            return Err(CliError::new(
                "hub_start_timeout",
                format!("the hub did not answer within {TIMEOUT:?}; it may still be starting"),
            )
            .hint(format!("mg hub status; {}", diagnostics_hint(&home)))
            .exit(exit::TIMEOUT));
        }
        std::thread::sleep(POLL);
    }
}

fn outcome(home: &Home, running: &Running, started: bool, warnings: Vec<String>) -> Outcome {
    let link = ui_link::of(running);
    let mut status = HubStatus {
        started: Some(started),
        ui_url: Some(link.url.clone()),
        ..running.status(home)
    };
    status.warnings.extend(warnings);
    let verb = if started {
        "started"
    } else {
        "already running"
    };
    let mut text = format!("hub {verb} at {}", running.describe());
    if let Some(service) = &status.service {
        text.push_str(&format!(
            "\nruns as a {} service: {}",
            service.manager.name(),
            service.unit
        ));
    }
    for warning in &status.warnings {
        text.push_str(&format!("\nwarning: {warning}"));
    }
    text.push_str(&format!("\n{}", link.text()));
    Outcome::done(&status, text)
}

fn run_here(home: Home, port: u16) -> Result<Outcome, CliError> {
    let status = HubStatus::stopped(&home, true);
    hub::run(home, port, env!("MG_VERSION")).map_err(hub_error)?;
    Ok(Outcome::done(&status, "hub stopped"))
}

fn incompatible(running: &Running) -> CliError {
    CliError::new(
        "hub_incompatible",
        format!(
            "the running hub speaks API {:08x}, this mg speaks {:08x}",
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
