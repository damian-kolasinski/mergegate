use hub::Home;

use super::probe::{live_pid, probe};
use super::report::HubStatus;
use crate::output::{CliError, Outcome};

pub(super) fn run(home: &Home) -> Result<Outcome, CliError> {
    let connected = home.read_connection().map(|c| c.url);
    // Commands go to that hub, whatever runs here.
    let note = |text: String| match &connected {
        Some(url) => format!("{text}\nthis mg works through the hub at {url} (mg connect)"),
        None => text,
    };
    if let Some(running) = probe(home) {
        let status = HubStatus {
            connected: connected.clone(),
            ..running.status(home)
        };
        let mut text = format!("hub running at {}", running.describe());
        if let Some(service) = &status.service {
            text.push_str(&format!(
                "\nruns as a {} service: {}",
                service.manager.name(),
                service.unit
            ));
        }
        return Ok(Outcome::done(&status, note(text)));
    }
    let unresponsive = home.is_locked();
    let status = HubStatus {
        unresponsive,
        pid: live_pid(home).filter(|_| unresponsive),
        connected: connected.clone(),
        ..HubStatus::new(home)
    };
    let text = if unresponsive {
        "hub not responding (a hub process holds the data directory)"
    } else {
        "hub not running"
    };
    Ok(Outcome::done(&status, note(text.to_owned())))
}
