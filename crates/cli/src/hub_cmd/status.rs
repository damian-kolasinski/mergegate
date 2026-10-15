use hub::Home;

use super::probe::probe;
use super::report::HubStatus;
use crate::output::{CliError, Outcome};

pub(super) fn run(home: &Home) -> Result<Outcome, CliError> {
    if let Some(running) = probe(home) {
        let status = running.status(home);
        let mut text = format!("hub running at {}", running.describe());
        if let Some(service) = &status.service {
            text.push_str(&format!(
                "\nruns as a {} service: {}",
                service.manager.name(),
                service.unit
            ));
        }
        return Ok(Outcome::done(&status, text));
    }
    let unresponsive = home.is_locked();
    let status = HubStatus {
        unresponsive,
        pid: home
            .read_info()
            .map(|info| info.pid)
            .filter(|_| unresponsive),
        ..HubStatus::new(home)
    };
    let text = if unresponsive {
        "hub not responding (a hub process holds the data directory)"
    } else {
        "hub not running"
    };
    Ok(Outcome::done(&status, text))
}
