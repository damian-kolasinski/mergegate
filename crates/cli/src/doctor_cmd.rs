//! `mg doctor`: whether the hub, git and every project are ready to work.

use api::doctor::{Check, CheckStatus, Report};
use serde::Serialize;

use crate::client::{self, Client};
use crate::output::{CliError, Outcome, exit};

#[derive(Serialize)]
struct Doctor {
    /// No check failed; warnings allowed.
    ok: bool,
    checks: Vec<Check>,
}

pub fn run() -> Result<Outcome, CliError> {
    let home = crate::home()?;
    let mut checks = Vec::new();
    // Everything else is checked by the hub, so a hub that can't be reached
    // or understood ends the report.
    match client::running(&home).and_then(|running| {
        let client = Client::to(&running)?;
        Ok((running, client))
    }) {
        Ok((running, client)) => {
            checks.push(Check {
                name: "hub".into(),
                status: CheckStatus::Ok,
                message: format!("running at {}", running.url()),
                fix: None,
            });
            let report: Report = client.get("/api/doctor")?;
            checks.extend(report.checks);
        }
        Err(e) => checks.push(Check {
            name: "hub".into(),
            status: CheckStatus::Fail,
            message: e.body.message,
            fix: e.body.hint,
        }),
    }
    let ok = !checks.iter().any(|c| c.status == CheckStatus::Fail);
    let text = text(&checks);
    let outcome = Outcome::done(&Doctor { ok, checks }, text);
    Ok(if ok {
        outcome
    } else {
        outcome.exit(exit::ERROR)
    })
}

fn text(checks: &[Check]) -> String {
    let width = checks.iter().map(|c| c.name.len()).max().unwrap_or(0);
    let mut out = Vec::new();
    for check in checks {
        let status = match check.status {
            CheckStatus::Ok => "ok",
            CheckStatus::Warn => "warn",
            CheckStatus::Fail => "FAIL",
        };
        out.push(format!(
            "{status:4}  {:width$}  {}",
            check.name, check.message
        ));
        if let Some(fix) = &check.fix {
            out.push(format!("{:4}  {:width$}  fix: {fix}", "", ""));
        }
    }
    out.join("\n")
}
