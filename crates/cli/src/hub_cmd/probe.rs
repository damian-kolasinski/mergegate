//! Finding the hub a home points to, and reporting on it.

use std::time::Duration;

use hub::{Home, HubInfo};
use serde::Serialize;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatible: Option<bool>,
    /// A hub process holds the home's lock but doesn't answer.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unresponsive: bool,
}

impl HubStatus {
    pub fn stopped(home: &Home, stopped: Option<bool>) -> Self {
        Self {
            home: home_str(home),
            stopped,
            ..Self::default()
        }
    }
}

pub(super) struct Running {
    info: HubInfo,
    pub health: api::hub::Health,
}

impl Running {
    pub fn url(&self) -> String {
        format!("http://{}", self.info.addr)
    }

    pub fn compatible(&self) -> bool {
        self.health.api_version == api::API_VERSION
    }

    pub fn status(&self, home: &Home) -> HubStatus {
        HubStatus {
            running: true,
            home: home_str(home),
            url: Some(self.url()),
            pid: Some(self.health.pid),
            version: Some(self.health.version.clone()),
            api_version: Some(self.health.api_version),
            schema_version: Some(self.health.schema_version),
            compatible: Some(self.compatible()),
            ..HubStatus::default()
        }
    }

    pub fn describe(&self) -> String {
        let h = &self.health;
        let mut text = format!(
            "{} (pid {}, mg {}, schema {})",
            self.url(),
            h.pid,
            h.version,
            h.schema_version
        );
        if !self.compatible() {
            text.push_str(&format!(
                "\nwarning: the hub speaks API v{}, this mg speaks v{}; \
                 restart it with this mg: mg hub stop && mg hub start",
                h.api_version,
                api::API_VERSION
            ));
        }
        text
    }
}

/// The hub named in `hub.json`, if it answers as that process. Home paths
/// aren't compared: symlinks give one directory many spellings.
pub(super) fn probe(home: &Home) -> Option<Running> {
    let info = home.read_info()?;
    let health: api::hub::Health = agent()
        .get(format!("http://{}/api/health", info.addr))
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    (health.pid == info.pid).then_some(Running { info, health })
}

pub(super) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .proxy(None)
        .build()
        .into()
}

pub(super) fn home_str(home: &Home) -> String {
    home.root().display().to_string()
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
