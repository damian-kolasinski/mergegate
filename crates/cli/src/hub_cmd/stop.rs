use std::time::{Duration, Instant};

use hub::Home;

use super::POLL;
use super::probe::{agent, probe};
use super::report::{HubStatus, unresponsive};
use crate::output::{CliError, Outcome, exit};

const TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn run(home: &Home) -> Result<Outcome, CliError> {
    // A hub that holds the lock without answering is starting or stopping;
    // give it until the deadline to do either.
    let deadline = Instant::now() + TIMEOUT;
    let running = loop {
        if let Some(running) = probe(home) {
            break running;
        }
        if !home.is_locked() {
            let status = HubStatus::stopped(home, false);
            return Ok(Outcome::done(&status, "hub not running"));
        }
        if Instant::now() > deadline {
            return Err(unresponsive(home));
        }
        std::thread::sleep(POLL);
    };

    let kill_hint = format!("kill {}", running.health.pid);
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
    let status = HubStatus::stopped(home, true);
    Ok(Outcome::done(&status, "hub stopped"))
}
