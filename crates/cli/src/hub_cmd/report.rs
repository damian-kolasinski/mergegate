//! What `mg hub` commands report.

use hub::Home;
use serde::Serialize;

use super::service::{self, Installed};
use crate::output::{CliError, exit};

#[derive(Serialize, Default)]
pub(super) struct HubStatus {
    pub running: bool,
    pub home: String,
    /// Whether this `start` started the hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<bool>,
    /// Whether this `stop` stopped the hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,
    /// Whether this `mg` speaks the running hub's API version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatible: Option<bool>,
    /// A hub process holds the home's lock but doesn't answer.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unresponsive: bool,
    /// The service unit that runs the default home's hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<Installed>,
    /// The unit this `stop` removed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_removed: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl HubStatus {
    pub fn new(home: &Home) -> Self {
        Self {
            home: home.root().display().to_string(),
            service: service::applies().then(service::installed).flatten(),
            ..Self::default()
        }
    }

    pub fn stopped(home: &Home, stopped: bool) -> Self {
        Self {
            stopped: Some(stopped),
            ..Self::new(home)
        }
    }
}

/// The error a hub started in the background printed as it exited.
pub(super) fn exited(printed: &[u8], home: &Home) -> CliError {
    // A service may have retried and printed one per attempt.
    let first = serde_json::Deserializer::from_slice(printed)
        .into_iter::<api::error::ErrorEnvelope>()
        .next();
    match first {
        Some(Ok(envelope)) => CliError {
            exit: exit::ERROR,
            body: envelope.error,
        },
        _ => CliError::new("hub_start_failed", "the hub exited while starting")
            .hint(format!("see {}", home.log().display())),
    }
}

pub(super) fn unresponsive(home: &Home) -> CliError {
    let hint = match home.read_info() {
        Some(info) => format!("kill {}, then mg hub start", info.pid),
        None => format!(
            "stop the process holding {}/hub.lock",
            home.root().display()
        ),
    };
    CliError::new(
        "hub_unresponsive",
        "a hub process holds the data directory but doesn't answer",
    )
    .hint(hint)
    .exit(exit::UNAVAILABLE)
}
