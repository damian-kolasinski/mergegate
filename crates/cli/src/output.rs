//! The CLI's output contract. Agents branch on exit codes and parse `--json`
//! output, so both are stable.
//!
//! With `--json` (or `MG_JSON=1`) every outcome is one JSON object on stdout:
//! the result, `{"needs_decision": {...}}` or `{"error": {...}}`. Without it,
//! results and decisions go to stdout as compact text and errors to stderr.

use std::process::ExitCode;

use api::decision::{NeedsDecision, NeedsDecisionEnvelope};
use api::error::{ErrorBody, ErrorEnvelope};
use serde::Serialize;
use serde_json::Value;

pub mod exit {
    pub const OK: u8 = 0;
    pub const ERROR: u8 = 1;
    /// The command needs the operator to choose; see `needs_decision`.
    pub const NEEDS_DECISION: u8 = 2;
    /// The item the command waits on was merged or abandoned.
    pub const ITEM_CLOSED: u8 = 3;
    /// The command line is invalid (sysexits `EX_USAGE`).
    pub const USAGE: u8 = 64;
    /// The hub is not running or not reachable (sysexits `EX_UNAVAILABLE`).
    pub const UNAVAILABLE: u8 = 69;
    /// A wait timed out with nothing to report; call it again (`EX_TEMPFAIL`).
    pub const TIMEOUT: u8 = 75;
}

pub const EXIT_CODES_HELP: &str = "\
Exit codes: 0 ok · 1 error · 2 needs a decision (re-run with the named flag) · \
3 item closed · 64 usage · 69 hub unavailable · 75 timed out (call again)";

pub enum Outcome {
    Done { json: Value, text: String },
    NeedsDecision(NeedsDecision),
}

impl Outcome {
    pub fn done(value: &impl Serialize, text: impl Into<String>) -> Self {
        Self::Done {
            json: serde_json::to_value(value).expect("output types serialize"),
            text: text.into(),
        }
    }
}

#[derive(Debug)]
pub struct CliError {
    pub exit: u8,
    pub body: ErrorBody,
}

impl CliError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            exit: exit::ERROR,
            body: ErrorBody {
                code: code.into(),
                message: message.into(),
                hint: None,
            },
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.body.hint = Some(hint.into());
        self
    }

    pub fn exit(mut self, exit: u8) -> Self {
        self.exit = exit;
        self
    }
}

/// Prints an outcome and returns the process exit code.
pub fn emit(json: bool, result: Result<Outcome, CliError>) -> ExitCode {
    let (stdout, stderr, code) = render(json, result);
    if !stdout.is_empty() {
        println!("{stdout}");
    }
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }
    ExitCode::from(code)
}

fn render(json: bool, result: Result<Outcome, CliError>) -> (String, String, u8) {
    match result {
        Ok(Outcome::Done { json: value, text }) => {
            let out = if json { value.to_string() } else { text };
            (out, String::new(), exit::OK)
        }
        Ok(Outcome::NeedsDecision(decision)) => {
            let out = if json {
                to_json(&NeedsDecisionEnvelope {
                    needs_decision: decision,
                })
            } else {
                decision_text(&decision)
            };
            (out, String::new(), exit::NEEDS_DECISION)
        }
        Err(CliError { exit, body }) => {
            if json {
                (to_json(&ErrorEnvelope { error: body }), String::new(), exit)
            } else {
                let mut err = format!("error: {}", body.message);
                if let Some(hint) = body.hint {
                    err.push_str(&format!("\nhint: {hint}"));
                }
                (String::new(), err, exit)
            }
        }
    }
}

fn decision_text(d: &NeedsDecision) -> String {
    let mut out = format!("needs decision: {}", d.question);
    for option in &d.options {
        out.push_str(&format!("\n  {} {}", d.flag, option.value));
        if d.default.as_deref() == Some(option.value.as_str()) {
            out.push_str(" (default)");
        }
        if let Some(description) = &option.description {
            out.push_str(&format!(" · {description}"));
        }
    }
    out.push_str(&format!(
        "\nre-run with {} set to the chosen option",
        d.flag
    ));
    out
}

fn to_json(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("output types serialize")
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
