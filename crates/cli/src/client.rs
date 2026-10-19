//! Requests to the running hub, for commands that work through it.

use std::time::Duration;

use api::decision::{NeedsDecision, NeedsDecisionEnvelope};
use api::error::ErrorEnvelope;
use hub::Home;
use serde::Serialize;
use serde::de::DeserializeOwned;
use ureq::http::Response;

use crate::hub_cmd::probe::settle;
use crate::hub_cmd::upgrade;
use crate::output::{CliError, exit};

/// How long a command waits for a hub that holds its lock but doesn't
/// answer yet, as one starting does while it opens and migrates its store.
/// Shorter than `mg hub start`'s wait: past it, a hung hub is reported.
const STARTING: Duration = Duration::from_secs(3);

/// How long a connected `mg` waits for the hub on another machine to answer.
/// Before it answers a device it hasn't seen in a minute, the hub asks the
/// `tailscale` CLI who it is and gives the CLI 5 seconds; a first request
/// also waits on MagicDNS and the WireGuard handshake. Shorter than that,
/// a working hub reads as unreachable.
const REACH: Duration = Duration::from_secs(10);

/// How long a connected `mg` waits to connect, within `REACH`. A hub machine
/// that is off the tailnet never refuses, so this is how soon it is reported.
const REACH_CONNECT: Duration = Duration::from_secs(5);

pub(crate) struct Client {
    url: String,
    agent: ureq::Agent,
    connected: bool,
}

/// The hub this `mg` works through, and what it said of itself.
pub(crate) struct Hub {
    pub url: String,
    pub health: api::hub::Health,
    /// It is on another machine, as `mg connect` set.
    pub connected: bool,
}

impl Hub {
    fn compatible(&self) -> bool {
        self.health.api_version == api::API_VERSION
    }
}

/// What the hub answered a request that may need a decision.
pub(crate) enum Reply<T> {
    Done(T),
    Decide(NeedsDecision),
}

impl Client {
    /// The hub `home` points to, if it runs and speaks this `mg`'s API.
    pub fn connect(home: &Home) -> Result<Self, CliError> {
        Self::to(&running(home)?)
    }

    pub fn to(hub: &Hub) -> Result<Self, CliError> {
        if !hub.compatible() {
            let message = format!(
                "the hub speaks API {:08x}, this mg speaks {:08x}",
                hub.health.api_version,
                api::API_VERSION
            );
            let hint = match hub.connected {
                true => "run the same mg here as on the hub's machine",
                false => "mg hub stop && mg hub start",
            };
            return Err(CliError::new("hub_incompatible", message)
                .hint(hint)
                .exit(exit::UNAVAILABLE));
        }
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(2)))
            // Registering a project fetches the whole repo into staging, which
            // takes as long as it takes; only a hub that stopped answering
            // ends a request.
            .proxy(None)
            .http_status_as_error(false)
            .build()
            .into();
        Ok(Self {
            url: hub.url.clone(),
            agent,
            connected: hub.connected,
        })
    }

    /// The hub is on another machine.
    pub fn connected(&self) -> bool {
        self.connected
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, CliError> {
        done(self.read(self.agent.get(self.url(path)).call()))
    }

    pub fn get_query<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, CliError> {
        let request = self
            .agent
            .get(self.url(path))
            .query_pairs(query.iter().copied());
        done(self.read(request.call()))
    }

    pub fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<Reply<T>, CliError> {
        let request = self
            .agent
            .post(self.url(path))
            .header(api::CLIENT_HEADER, "mg");
        self.read(request.send_json(body))
    }

    pub fn patch<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<Reply<T>, CliError> {
        let request = self
            .agent
            .patch(self.url(path))
            .header(api::CLIENT_HEADER, "mg");
        self.read(request.send_json(body))
    }

    pub fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T, CliError> {
        let request = self
            .agent
            .delete(self.url(path))
            .header(api::CLIENT_HEADER, "mg");
        done(self.read(request.call()))
    }

    pub fn put<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, CliError> {
        let request = self
            .agent
            .put(self.url(path))
            .header(api::CLIENT_HEADER, "mg");
        done(self.read(request.send_json(body)))
    }

    /// Sends `body` as it is, as a file upload.
    pub fn post_bytes<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        body: Vec<u8>,
    ) -> Result<T, CliError> {
        let request = self
            .agent
            .post(self.url(path))
            .query_pairs(query.iter().copied())
            .header(api::CLIENT_HEADER, "mg")
            .header("content-type", "application/octet-stream");
        done(self.read(request.send(&body[..])))
    }

    pub fn remote(&self, project: &str) -> String {
        format!("{}/git/{}", self.url, segment(project))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.url)
    }

    fn read<T: DeserializeOwned>(
        &self,
        response: Result<Response<ureq::Body>, ureq::Error>,
    ) -> Result<Reply<T>, CliError> {
        let lost = |e: ureq::Error| {
            CliError::new(
                "hub_unavailable",
                format!("lost the hub at {}: {e}", self.url),
            )
            .hint(match self.connected {
                true => "mg doctor",
                false => "mg hub status",
            })
            .exit(exit::UNAVAILABLE)
        };
        let mut response = response.map_err(lost)?;
        let status = response.status();
        let body = response.body_mut().read_to_string().map_err(lost)?;
        let unexpected = || {
            CliError::new(
                "unexpected_reply",
                format!("the hub answered {status}: {body}"),
            )
        };
        if status.is_success() {
            return serde_json::from_str(&body)
                .map(Reply::Done)
                .map_err(|_| unexpected());
        }
        if let Ok(envelope) = serde_json::from_str::<NeedsDecisionEnvelope>(&body) {
            return Ok(Reply::Decide(envelope.needs_decision));
        }
        match serde_json::from_str::<ErrorEnvelope>(&body) {
            Ok(envelope) => Err(CliError {
                exit: exit::ERROR,
                body: envelope.error,
            }),
            Err(_) => Err(unexpected()),
        }
    }
}

/// The hub `mg connect` named, or else the one `home` runs, if it answers.
pub(crate) fn running(home: &Home) -> Result<Hub, CliError> {
    if let Some(connection) = home.read_connection() {
        return reach(&connection.url);
    }
    // A hub that holds the lock without answering is not "not running":
    // `mg hub start` refuses while it holds it, so hinting that would send
    // the agent in a circle.
    let running = match settle(home, STARTING)? {
        Some(running) => upgrade::onto_this_mg(home, running)?,
        None => None,
    };
    let running = running.ok_or_else(|| {
        CliError::new("hub_unavailable", "the hub is not running")
            .hint("mg hub start")
            .exit(exit::UNAVAILABLE)
    })?;
    Ok(Hub {
        url: running.url(),
        health: running.health,
        connected: false,
    })
}

/// The hub at `url`, on another machine.
pub(crate) fn reach(url: &str) -> Result<Hub, CliError> {
    let unreachable = |why: String| {
        CliError::new(
            "hub_unavailable",
            format!("can't reach the hub at {url}: {why}"),
        )
        .hint("check Tailscale on both machines, or use this machine's hub: mg connect --local")
        .exit(exit::UNAVAILABLE)
    };
    // A refusal is the hub's to explain: which device it refused and how the
    // operator lets it in.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(REACH))
        .timeout_connect(Some(REACH_CONNECT))
        .proxy(None)
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent
        .get(format!("{url}/api/health"))
        .call()
        .map_err(|e| unreachable(e.to_string()))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.body_mut().read_to_string().unwrap_or_default();
        return Err(match serde_json::from_str::<ErrorEnvelope>(&body) {
            Ok(envelope) => CliError {
                exit: exit::ERROR,
                body: envelope.error,
            },
            Err(_) => unreachable(format!("it answered {status}")),
        });
    }
    let health = response
        .body_mut()
        .read_json()
        .map_err(|e| unreachable(format!("it doesn't answer as a mergegate hub ({e})")))?;
    Ok(Hub {
        url: url.to_owned(),
        health,
        connected: true,
    })
}

/// For requests that never answer with a decision.
fn done<T>(reply: Result<Reply<T>, CliError>) -> Result<T, CliError> {
    match reply? {
        Reply::Done(value) => Ok(value),
        Reply::Decide(decision) => Err(CliError::new(
            "unexpected_reply",
            format!("the hub asked: {}", decision.question),
        )),
    }
}

/// A path segment, such as a project name typed by the operator, that can't
/// reach another route. Dots are escaped too, so `..` stays a name rather
/// than a step up.
pub(crate) fn segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
