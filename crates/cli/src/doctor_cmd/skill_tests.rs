use std::path::PathBuf;

use api::doctor::CheckStatus;
use skill::{Agent, AgentStatus, Item, State};

use super::check;

fn agent(agent: Agent, found: bool, skill: State, hook: State) -> AgentStatus {
    let item = |state| Item {
        path: Some(PathBuf::from("/somewhere")),
        state,
    };
    AgentStatus {
        agent,
        found,
        skill: item(skill),
        hook: item(hook),
    }
}

#[test]
fn no_agent_found_has_nothing_to_set_up() {
    let status = [agent(Agent::Claude, false, State::Missing, State::Missing)];
    let check = check(&status);
    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(check.message, "found no agent to set up (Claude Code)");
}

#[test]
fn an_agent_found_without_its_skill_warns_with_setup() {
    let status = [
        agent(Agent::Claude, true, State::Current, State::Current),
        agent(Agent::Codex, true, State::Outdated, State::Missing),
        agent(Agent::OpenCode, false, State::Missing, State::Unsupported),
    ];
    let check = check(&status);
    assert_eq!(check.status, CheckStatus::Warn);
    assert_eq!(
        check.message,
        "not set up in Codex (skill from another mg, hook missing)"
    );
    assert_eq!(check.fix.as_deref(), Some("mg skill setup"));
}

#[test]
fn every_agent_found_set_up_is_ok() {
    let status = [
        agent(Agent::Claude, true, State::Current, State::Current),
        agent(Agent::OpenCode, true, State::Current, State::Unsupported),
    ];
    let check = check(&status);
    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(check.message, "set up in Claude Code, OpenCode");
}
