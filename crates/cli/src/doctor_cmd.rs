//! `mg doctor`: whether the hub, its service, git, every project, the skill
//! and this clone are ready to work.

mod hub;
mod remote;
mod service;
mod skill;

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
    let connected = client::running(&home).and_then(|running| {
        let client = Client::to(&running)?;
        Ok((running, client))
    });
    checks.push(match &connected {
        Ok((running, _)) => hub::check(&running.url(), &running.health.version),
        Err(e) => Check {
            name: "hub".into(),
            status: CheckStatus::Fail,
            message: e.body.message.clone(),
            fix: e.body.hint.clone(),
        },
    });
    checks.extend(service::check(connected.is_ok()));
    // The rest is checked by the hub, so a hub that can't be reached or
    // understood skips it.
    if let Ok((_, client)) = &connected {
        let report: Report = client.get("/api/doctor")?;
        checks.extend(report.checks);
    }
    checks.push(skill::check(
        &home.root().join("skill").join(::skill::NAME),
        ::skill::FILES,
    ));
    if let Ok((running, client)) = &connected {
        checks.extend(remote::check(running, client)?);
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
