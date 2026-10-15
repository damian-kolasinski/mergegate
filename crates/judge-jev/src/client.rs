//! Requests to `/v1/systemone`, retried while Jev is busy.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use mergegate_core::judge::JudgeError;
use serde_json::{Value, json};
use ureq::Agent;

/// Busy answers retried before giving up: waits of 1, 2, 4, 8 and 16
/// backoffs.
const RETRIES: u32 = 5;

pub(crate) struct Client {
    base: String,
    agent: Agent,
    backoff: Duration,
}

impl Client {
    pub fn new(base: String, backoff: Duration) -> Self {
        let agent = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .user_agent("mergegate")
            .build()
            .into();
        Self {
            base,
            agent,
            backoff,
        }
    }

    /// The `answers` object, keyed as `questions` is. A request under way
    /// when `stop` is set runs out its time; no other starts.
    pub fn ask(
        &self,
        key: &str,
        model: &str,
        state: &str,
        questions: Value,
        stop: &AtomicBool,
    ) -> Result<Value, JudgeError> {
        let body = json!({ "model": model, "state": state, "questions": questions });
        let url = format!("{}/v1/systemone", self.base);
        let failed = |e: &dyn std::fmt::Display| JudgeError(format!("reaching Jev failed: {e}"));
        for attempt in 0..=RETRIES {
            if stop.load(Ordering::Relaxed) {
                return Err(JudgeError("asking Jev was stopped with the hub".into()));
            }
            let response = self
                .agent
                .post(&url)
                .header("Authorization", format!("Bearer {key}"))
                .send_json(&body);
            let mut response = response.map_err(|e| failed(&e))?;
            let status = response.status().as_u16();
            let text = response
                .body_mut()
                .read_to_string()
                .map_err(|e| failed(&e))?;
            let reply: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            match status {
                200..300 => {
                    let answers = &reply["answers"];
                    if !answers.is_object() {
                        return Err(JudgeError("Jev's answer has no answers".into()));
                    }
                    return Ok(answers.clone());
                }
                429 | 529 if attempt < RETRIES => {
                    pause(self.backoff * 2u32.pow(attempt), stop);
                }
                401 => {
                    return Err(JudgeError(format!(
                        "Jev turned down the key in {}",
                        crate::KEY_VAR
                    )));
                }
                status => {
                    return Err(JudgeError(format!(
                        "Jev answered {status}: {}",
                        message(&reply, &text)
                    )));
                }
            }
        }
        unreachable!("the last attempt returns")
    }
}

/// Sleeps `wait`, or until `stop` is set.
fn pause(wait: Duration, stop: &AtomicBool) {
    let until = Instant::now() + wait;
    while !stop.load(Ordering::Relaxed) {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return;
        }
        thread::sleep(left.min(Duration::from_millis(50)));
    }
}

/// The error's message, wherever the reply puts it.
fn message(reply: &Value, text: &str) -> String {
    [
        &reply["error"]["message"],
        &reply["message"],
        &reply["error"],
    ]
    .into_iter()
    .find_map(Value::as_str)
    .map_or_else(|| text.trim().chars().take(200).collect(), str::to_owned)
}
