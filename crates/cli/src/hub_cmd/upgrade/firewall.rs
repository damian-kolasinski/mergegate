//! The macOS firewall after a restart onto a new `mg`. It keeps a rule per
//! binary, and to it an `mg` without a Developer ID signature is a new one
//! on every upgrade: other machines' connections time out until the operator
//! allows it again. The hub sees that, as `mg doctor` shows; this says so
//! where the upgrade happened, with the command that fixes it.

use std::time::Duration;

use api::doctor::{CheckStatus, Report};

use super::super::probe::Running;
use crate::client::{Client, Hub};

/// The doctor also asks GitHub, which can take a while.
const ASK: Duration = Duration::from_secs(30);

/// Prints what keeps the tailnet from the hub `running`, if anything. A hub
/// that can't say stays quiet: the restart already happened.
pub(super) fn warn(running: &Running) {
    // Only macOS has the firewall, and the doctor's other checks would hold
    // up the command that restarted the hub.
    if !cfg!(target_os = "macos") {
        return;
    }
    let hub = Hub {
        url: running.url(),
        health: running.health.clone(),
        connected: false,
    };
    let Ok(client) = Client::to(&hub) else {
        return;
    };
    let Ok(report) = client.time_limit(ASK).get::<Report>("/api/doctor") else {
        return;
    };
    for warning in warnings(report) {
        eprintln!("{warning}");
    }
}

fn warnings(report: Report) -> Vec<String> {
    report
        .checks
        .into_iter()
        .filter(|c| c.name == "firewall" && c.status != CheckStatus::Ok)
        .map(|c| match c.fix {
            Some(fix) => format!("warning: {}; on this machine, run: {fix}", c.message),
            None => format!("warning: {}", c.message),
        })
        .collect()
}

#[cfg(test)]
#[path = "firewall_tests.rs"]
mod tests;
