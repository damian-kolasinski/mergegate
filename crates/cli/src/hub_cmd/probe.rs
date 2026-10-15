//! Finding and talking to the hub a home points to.

use std::time::{Duration, Instant};

use hub::{Home, HubInfo};

use super::POLL;
use super::report::{HubStatus, unresponsive};
use crate::output::CliError;

pub(crate) struct Running {
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

    pub(super) fn status(&self, home: &Home) -> HubStatus {
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
pub(crate) fn probe(home: &Home) -> Option<Running> {
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

/// The hub that answers within `patience`, or `None` once no hub holds the
/// home's lock. A hub takes the lock before it opens its store and answers,
/// and lets go of it after it stops answering, so one that holds it without
/// answering may be starting or stopping; past `patience` it is reported
/// unresponsive.
pub(crate) fn settle(home: &Home, patience: Duration) -> Result<Option<Running>, CliError> {
    let deadline = Instant::now() + patience;
    loop {
        if let Some(running) = probe(home) {
            return Ok(Some(running));
        }
        if !home.is_locked() {
            return Ok(None);
        }
        if Instant::now() > deadline {
            return Err(unresponsive(home));
        }
        std::thread::sleep(POLL);
    }
}

/// The pid `hub.json` names, if a process has it. A hub killed outright
/// leaves its `hub.json` behind, and the pid in it may be gone or reused.
pub(crate) fn live_pid(home: &Home) -> Option<u32> {
    let pid = home.read_info()?.pid;
    alive(pid).then_some(pid)
}

#[allow(unsafe_code)]
fn alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // pid 0 and negative pids name process groups, not a process.
    if pid <= 0 {
        return false;
    }
    // SAFETY: signal 0 sends nothing; kill only checks that the process
    // exists and may be signalled.
    let sent = unsafe { libc::kill(pid, 0) };
    // EPERM: it exists but belongs to another user.
    sent == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

pub(super) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .proxy(None)
        .build()
        .into()
}
