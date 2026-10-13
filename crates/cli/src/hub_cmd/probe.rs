//! Finding and talking to the hub a home points to.

use std::time::Duration;

use hub::{Home, HubInfo};

use super::report::HubStatus;

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
            url: Some(self.url()),
            pid: Some(self.health.pid),
            version: Some(self.health.version.clone()),
            api_version: Some(self.health.api_version),
            schema_version: Some(self.health.schema_version),
            compatible: Some(self.compatible()),
            ..HubStatus::new(home)
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
