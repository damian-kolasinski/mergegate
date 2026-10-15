//! Jev, TypeSafe's judgment model, over its System One API. The key is read
//! from the hub's environment on each call and never stored.

mod ask;
mod client;
mod secrets;
mod state;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use mergegate_core::judge::{Judge, JudgeError, Request, Verdict};

use client::Client;

/// The variable the key is read from, as TypeSafe's own SDKs read it.
pub const KEY_VAR: &str = "TYPESAFE_API_KEY";

pub struct Jev {
    client: Client,
    model: String,
    /// Set when the hub stops; no retry follows it.
    stop: Arc<AtomicBool>,
}

impl Jev {
    /// `base` as in `https://api.typesafe.ai`, without a trailing slash;
    /// `backoff` is the first wait after Jev says it's busy, doubled on each
    /// retry.
    pub fn new(base: String, model: String, backoff: Duration) -> Self {
        Self {
            client: Client::new(base, backoff),
            model,
            stop: Arc::default(),
        }
    }

    /// Gives up, rather than ask or wait to ask again, once `stop` is set.
    pub fn stopping_on(self, stop: Arc<AtomicBool>) -> Self {
        Self { stop, ..self }
    }

    /// TypeSafe's API, unless `MG_JEV_URL` points elsewhere, as tests point
    /// it at a fake; `MG_JEV_BACKOFF_MS` shortens the backoff for them.
    pub fn from_env(model: String) -> Self {
        let base = std::env::var("MG_JEV_URL")
            .ok()
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| "https://api.typesafe.ai".into());
        let backoff = std::env::var("MG_JEV_BACKOFF_MS")
            .ok()
            .and_then(|ms| ms.parse().ok())
            .unwrap_or(1000);
        Self::new(
            base.trim_end_matches('/').into(),
            model,
            Duration::from_millis(backoff),
        )
    }
}

/// Why Jev can't be called, before calling it.
pub fn missing_key() -> Option<String> {
    let set = std::env::var(KEY_VAR).is_ok_and(|key| !key.trim().is_empty());
    (!set).then(|| format!("{KEY_VAR} isn't set in the hub's environment"))
}

impl Judge for Jev {
    fn judge(&self, request: &Request) -> Result<Vec<Verdict>, JudgeError> {
        if let Some(problem) = missing_key() {
            return Err(JudgeError(problem));
        }
        let key = std::env::var(KEY_VAR).unwrap_or_default();
        let mut verdicts = Vec::new();
        for part in state::parts(request)? {
            let questions = ask::questions(request, &part.files);
            let answers =
                self.client
                    .ask(key.trim(), &self.model, &part.state, questions, &self.stop)?;
            verdicts.extend(ask::verdicts(request, &part.files, &answers));
        }
        Ok(verdicts)
    }
}
