use std::time::{Duration, Instant};

use hub::Home;

use super::POLL;
use super::probe::{agent, settle};
use super::report::HubStatus;
use super::{service, upgrade};
use crate::output::{CliError, Outcome, exit};

const TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn run(home: &Home) -> Result<Outcome, CliError> {
    let Some(running) = settle(home, TIMEOUT)? else {
        return stopped(home, false);
    };
    // Stopping removes the service, and this mg can't start the hub again on
    // the store a later release migrated: nothing would run until someone
    // found the hub's mg.
    if running.health.schema_version > hub::SCHEMA_VERSION {
        let theirs = &running.health.version;
        return Err(CliError::new(
            "hub_newer",
            format!(
                "the hub runs mg {theirs}, whose store this mg {} can't open",
                env!("MG_VERSION")
            ),
        )
        .hint(upgrade::older_mg(theirs)));
    }

    let kill_hint = format!("kill {}", running.health.pid);
    // The hub exits 0, which its service doesn't restart.
    agent()
        .post(format!("{}/api/hub/shutdown", running.url()))
        .header(api::CLIENT_HEADER, env!("CARGO_PKG_VERSION"))
        .send_empty()
        .map_err(|e| {
            CliError::new(
                "hub_unreachable",
                format!("asking the hub to stop failed: {e}"),
            )
            .hint(&kill_hint)
            .exit(exit::UNAVAILABLE)
        })?;
    let deadline = Instant::now() + TIMEOUT;
    while home.is_locked() {
        if Instant::now() > deadline {
            return Err(CliError::new(
                "hub_stop_timeout",
                format!("the hub did not stop within {TIMEOUT:?}"),
            )
            .hint(kill_hint)
            .exit(exit::TIMEOUT));
        }
        std::thread::sleep(POLL);
    }
    stopped(home, true)
}

/// Also removes the hub's service, so a stopped hub stays stopped across
/// logins.
fn stopped(home: &Home, stopped: bool) -> Result<Outcome, CliError> {
    let removed = if service::applies() {
        service::remove(service::Role::Hub)?
    } else {
        None
    };
    let mut text = if stopped {
        "hub stopped"
    } else {
        "hub not running"
    }
    .to_owned();
    if let Some(removed) = &removed {
        text.push_str(&format!("; removed its {} service", removed.manager.name()));
    }
    let status = HubStatus {
        service_removed: removed.map(|r| r.unit),
        ..HubStatus::stopped(home, stopped)
    };
    Ok(Outcome::done(&status, text))
}
