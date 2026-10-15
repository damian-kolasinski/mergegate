//! `mg doctor`: whether the hub, its service, git, every project, the skill
//! and this clone are ready to work.

mod hub;
mod remote;
mod service;
mod skill;

use api::device::{Trust, WhoAmI};
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
    // A device the hub refuses can't ask it anything else, so it fails the
    // hub check as an unreachable hub does.
    let connected = client::running(&home).and_then(|running| {
        let client = Client::to(&running)?;
        let me = match running.connected {
            true => Some(me(&client.get("/api/whoami")?)),
            false => None,
        };
        Ok((running, client, me))
    });
    checks.push(match &connected {
        Ok((running, _, None)) => hub::check(&running.url, &running.health.version),
        Ok((running, _, Some(me))) => Check {
            name: "hub".into(),
            status: CheckStatus::Ok,
            message: format!("connected to {} as {me}", running.url),
            fix: None,
        },
        Err(e) => Check {
            name: "hub".into(),
            status: CheckStatus::Fail,
            message: e.body.message.clone(),
            fix: e.body.hint.clone(),
        },
    });
    // The service starts this machine's hub, which a connected mg doesn't use.
    if home.read_connection().is_none() {
        checks.extend(service::check(connected.is_ok()));
    }
    // The rest is checked by the hub, so a hub that can't be reached or
    // understood skips it.
    if let Ok((_, client, _)) = &connected {
        let report: Report = client.get("/api/doctor")?;
        checks.extend(report.checks);
    }
    checks.push(skill::check(
        &home.root().join("skill").join(::skill::NAME),
        ::skill::FILES,
    ));
    if let Ok((running, client, _)) = &connected {
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

fn me(whoami: &WhoAmI) -> String {
    match &whoami.device {
        Some(d) if d.trust == Trust::Owner => format!("{}, the hub owner's device", d.name),
        Some(d) => format!("{}, allowed by the operator", d.name),
        None => "this machine".to_owned(),
    }
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
