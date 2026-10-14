//! Requests to GitHub's REST API, signed in as the operator.

use std::path::PathBuf;
use std::time::Duration;

use mergegate_core::github::GithubError;
use serde_json::Value;
use ureq::Agent;
use ureq::http::{Method, Request};

use crate::token::Token;

pub(crate) struct Api {
    base: String,
    agent: Agent,
    token: Token,
}

/// A response with its status, which callers map to their errors; 401 never
/// reaches them.
pub(crate) struct Reply {
    pub status: u16,
    pub body: Value,
}

impl Reply {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// GitHub's message, with the details it gives for a validation error.
    pub fn message(&self) -> String {
        let message = self.body["message"].as_str().unwrap_or("no message");
        let details: Vec<&str> = self.body["errors"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|error| error["message"].as_str())
            .collect();
        if details.is_empty() {
            message.to_owned()
        } else {
            format!("{message}: {}", details.join("; "))
        }
    }

    /// The error for a status the caller has no better meaning for.
    pub fn error(&self) -> GithubError {
        match self.status {
            403 | 405 | 409 | 422 => GithubError::Refused(self.message()),
            status => GithubError::Failed(format!("GitHub answered {status}: {}", self.message())),
        }
    }
}

impl Api {
    pub fn new(base: String, gh: PathBuf) -> Self {
        let agent = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent("mergegate")
            .build()
            .into();
        Self {
            base,
            agent,
            token: Token::new(gh),
        }
    }

    pub fn token(&self) -> &Token {
        &self.token
    }

    pub fn get(&self, path: &str) -> Result<Reply, GithubError> {
        self.call(Method::GET, path, None)
    }

    pub fn send(&self, method: Method, path: &str, body: Value) -> Result<Reply, GithubError> {
        self.call(method, path, Some(body))
    }

    pub fn delete(&self, path: &str) -> Result<Reply, GithubError> {
        self.call(Method::DELETE, path, None)
    }

    /// A token GitHub turns down is read from `gh` again and tried once
    /// more, so a `gh auth login` takes effect without restarting the hub.
    fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<Reply, GithubError> {
        for retry in [false, true] {
            if retry {
                self.token.forget();
            }
            let reply = self.once(method.clone(), path, body.as_ref())?;
            if reply.status != 401 {
                return Ok(reply);
            }
        }
        Err(GithubError::NotLoggedIn(
            "GitHub turned down the token gh gave".into(),
        ))
    }

    fn once(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Reply, GithubError> {
        let token = self.token.get()?;
        let request = Request::builder()
            .method(method)
            .uri(format!("{}{path}", self.base))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("Authorization", format!("Bearer {}", token.expose()));
        let failed =
            |e: &dyn std::fmt::Display| GithubError::Failed(format!("reaching GitHub failed: {e}"));
        let response = match body {
            Some(body) => {
                let request = request
                    .header("Content-Type", "application/json")
                    .body(body.to_string())
                    .map_err(|e| failed(&e))?;
                self.agent.run(request)
            }
            None => self.agent.run(request.body(()).map_err(|e| failed(&e))?),
        };
        let mut response = response.map_err(|e| failed(&e))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| failed(&e))?;
        // A 204 has no body; anything else not JSON says nothing more.
        let body = serde_json::from_str(&text).unwrap_or(Value::Null);
        Ok(Reply { status, body })
    }
}

/// `value` for a URL path segment or query value. Keeps `/`, which GitHub
/// takes as is in branch names.
pub(crate) fn encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
