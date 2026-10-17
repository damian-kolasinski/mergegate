//! A judge the operator runs as a command, e.g. `claude -p` or a script in
//! front of a local model: the change goes in as JSON on stdin, the verdicts
//! come back as JSON on stdout.

mod run;
mod wire;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use mergegate_core::judge::{Judge, JudgeError, Request, Verdict};

pub struct CommandJudge {
    command: String,
    /// The project's repo, so a command can read more than the diff.
    dir: PathBuf,
    timeout: Duration,
    /// Set when the hub stops; the command is stopped with it.
    stop: Arc<AtomicBool>,
}

impl CommandJudge {
    pub fn new(command: String, dir: PathBuf, timeout: Duration) -> Self {
        Self {
            command,
            dir,
            timeout,
            stop: Arc::default(),
        }
    }

    /// Stops the command once `stop` is set.
    pub fn stopping_on(self, stop: Arc<AtomicBool>) -> Self {
        Self { stop, ..self }
    }

    /// Five minutes, unless `MG_JUDGE_TIMEOUT_MS` says otherwise, as tests
    /// shorten it.
    pub fn from_env(command: String, dir: PathBuf) -> Self {
        let ms = std::env::var("MG_JUDGE_TIMEOUT_MS")
            .ok()
            .and_then(|ms| ms.parse().ok())
            .unwrap_or(300_000);
        Self::new(command, dir, Duration::from_millis(ms))
    }
}

impl Judge for CommandJudge {
    fn judge(&self, request: &Request) -> Result<Vec<Verdict>, JudgeError> {
        let input = serde_json::to_vec(&wire::input(request))
            .map_err(|e| JudgeError(format!("encoding the judge's input failed: {e}")))?;
        let output = run::run(
            &self.command,
            &self.dir,
            request,
            &input,
            self.timeout,
            &self.stop,
        )?;
        wire::verdicts(&output)
    }
}
