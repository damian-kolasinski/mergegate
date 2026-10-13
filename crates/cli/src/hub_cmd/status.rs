use hub::Home;

use super::probe::{HubStatus, home_str, probe};
use crate::output::{CliError, Outcome};

pub(super) fn run(home: &Home) -> Result<Outcome, CliError> {
    if let Some(running) = probe(home) {
        return Ok(Outcome::done(
            &running.status(home),
            format!("hub running at {}", running.describe()),
        ));
    }
    let unresponsive = home.is_locked();
    let status = HubStatus {
        home: home_str(home),
        unresponsive,
        pid: home
            .read_info()
            .map(|info| info.pid)
            .filter(|_| unresponsive),
        ..HubStatus::default()
    };
    let text = if unresponsive {
        "hub not responding (a hub process holds the data directory)"
    } else {
        "hub not running"
    };
    Ok(Outcome::done(&status, text))
}
