//! Whether the coding agents found on this machine have the skill stub and
//! session hook `mg skill setup` installs, as this `mg` installs them.

use api::doctor::{Check, CheckStatus};
use skill::{AgentStatus, Item, State};

pub(super) fn check(status: &[AgentStatus]) -> Check {
    let check = |status, message: String, fix: Option<&str>| Check {
        name: "skill".into(),
        status,
        message,
        fix: fix.map(str::to_owned),
    };
    let found: Vec<_> = status.iter().filter(|agent| agent.found).collect();
    if found.is_empty() {
        let known: Vec<_> = status.iter().map(|agent| agent.agent.label()).collect();
        return check(
            CheckStatus::Ok,
            format!("found no agent to set up ({})", known.join(", ")),
            None,
        );
    }
    // Setup fixes what's missing or another mg's; a skill that isn't mg's,
    // or settings it can't read, are the operator's to sort out.
    let fixable = found.iter().any(|agent| {
        [&agent.skill, &agent.hook]
            .iter()
            .any(|item| matches!(item.state, State::Missing | State::Outdated))
    });
    let unready: Vec<String> = found
        .iter()
        .filter(|agent| !agent.ready())
        .map(|agent| {
            let wrong: Vec<String> = [("skill", &agent.skill), ("hook", &agent.hook)]
                .into_iter()
                .filter_map(|(kind, item)| wrong(item).map(|why| format!("{kind} {why}")))
                .collect();
            format!("{} ({})", agent.agent.label(), wrong.join(", "))
        })
        .collect();
    if unready.is_empty() {
        let labels: Vec<_> = found.iter().map(|agent| agent.agent.label()).collect();
        return check(
            CheckStatus::Ok,
            format!("set up in {}", labels.join(", ")),
            None,
        );
    }
    check(
        CheckStatus::Warn,
        format!("not set up in {}", unready.join(", ")),
        fixable.then_some("mg skill setup"),
    )
}

fn wrong(item: &Item) -> Option<&'static str> {
    match item.state {
        State::Current | State::Unsupported => None,
        State::Outdated => Some("from another mg"),
        State::Missing => Some("missing"),
        State::Foreign => Some("not mg's"),
        State::Unreadable(_) => Some("unreadable"),
    }
}

#[cfg(test)]
#[path = "skill_tests.rs"]
mod tests;
