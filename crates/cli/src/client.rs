//! Requests to the running hub, for commands that work through it.

use std::time::Duration;

use api::decision::{NeedsDecision, NeedsDecisionEnvelope};
use api::error::ErrorEnvelope;
use hub::Home;
use serde::Serialize;
use serde::de::DeserializeOwned;
use ureq::http::Response;

use crate::hub_cmd::probe::{Running, probe};
use crate::output::{CliError, exit};

pub(crate) struct Client {
    url: String,
    agent: ureq::Agent,
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

    pub fn to(running: &Running) -> Result<Self, CliError> {
        if !running.compatible() {
            return Err(CliError::new(
                "hub_incompatible",
                format!(
                    "the hub speaks API v{}, this mg speaks v{}",
                    running.health.api_version,
                    api::API_VERSION
                ),
            )
            .hint("mg hub stop && mg hub start")
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
            url: running.url(),
            agent,
        })
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
    ) -> Result<T, CliError> {
        let request = self
            .agent
            .patch(self.url(path))
            .header(api::CLIENT_HEADER, "mg");
        done(self.read(request.send_json(body)))
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
            .hint("mg hub status")
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

/// The hub `home` points to, if it answers.
pub(crate) fn running(home: &Home) -> Result<Running, CliError> {
    probe(home).ok_or_else(|| {
        CliError::new("hub_unavailable", "the hub is not running")
            .hint("mg hub start")
            .exit(exit::UNAVAILABLE)
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
