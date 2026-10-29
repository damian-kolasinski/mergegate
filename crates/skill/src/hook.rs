//! The session hook: a `SessionStart` command that has `mergegate skill
//! session-start` tell each new session, and each one after a clear or a
//! compaction, how this repository works with mergegate. Claude Code and
//! Codex read the same shape: `hooks.SessionStart` holds groups, each with a
//! `hooks` list of handlers.

use crate::agent::Agent;
use crate::json::Json;

/// What every command this hook ran contains, however an older `mg` spelled
/// the rest, so setup finds its own among the operator's.
const MARKER: &str = "mergegate skill session-start";

/// Seconds an agent gives the hook; `session-start` keeps within it.
const TIMEOUT: u64 = 5;

/// The command, which must stay byte for byte the same across releases:
/// Codex trusts a hook by its hash, so a changed one stops running until
/// the operator trusts it again. `mergegate`, not `mg`, which macOS ships as
/// an editor; the shell keeps a missing `mergegate` or a failure quiet, so a
/// session starts all the same.
pub(crate) fn command(agent: Agent) -> String {
    format!(
        "/bin/sh -c '{MARKER} --agent {} 2>/dev/null || true'",
        agent.id()
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookState {
    Current,
    /// Ours, but not as this `mg` writes it, or written more than once.
    Outdated,
    Missing,
}

/// Where `settings` stands for `agent`'s hook.
pub(crate) fn state(settings: &Json, agent: Agent) -> HookState {
    let groups = session_start(settings);
    let ours = groups.iter().filter(|group| has_ours(group)).count();
    match ours {
        0 => HookState::Missing,
        1 if groups.contains(&group(agent)) => HookState::Current,
        _ => HookState::Outdated,
    }
}

/// Replaces every hook of ours in `settings` with this `mg`'s, last among
/// the session's hooks. `Err` names what isn't the shape agents read, which
/// is left as it was.
pub(crate) fn install(settings: &mut Json, agent: Agent) -> Result<(), &'static str> {
    remove(settings)?;
    let hooks = settings
        .entry("hooks")
        .ok_or("the settings aren't a JSON object")?;
    let start = hooks
        .entry("SessionStart")
        .ok_or("`hooks` isn't an object")?;
    if let Json::Object(fields) = start
        && fields.is_empty()
    {
        *start = Json::Array(Vec::new());
    }
    match start {
        Json::Array(groups) => groups.push(group(agent)),
        _ => return Err("`hooks.SessionStart` isn't a list"),
    }
    Ok(())
}

/// Takes our handlers out of `settings`, and the groups, lists and objects
/// that held only them. Whether anything was ours.
pub(crate) fn remove(settings: &mut Json) -> Result<bool, &'static str> {
    let Json::Object(_) = settings else {
        return Err("the settings aren't a JSON object");
    };
    let Some(hooks) = settings.get("hooks") else {
        return Ok(false);
    };
    let Some(start) = hooks.get("SessionStart") else {
        return Ok(false);
    };
    let Some(groups) = start.as_array() else {
        return Err("`hooks.SessionStart` isn't a list");
    };
    if !groups.iter().any(has_ours) {
        return Ok(false);
    }
    let kept: Vec<Json> = groups
        .iter()
        .filter_map(|group| without_ours(group.clone()))
        .collect();
    // Every lookup above found these.
    let hooks = settings.entry("hooks").expect("an object");
    match kept.is_empty() {
        true => hooks.remove("SessionStart"),
        false => *hooks.entry("SessionStart").expect("an object") = Json::Array(kept),
    }
    if hooks.is_empty_container() {
        settings.remove("hooks");
    }
    Ok(true)
}

fn session_start(settings: &Json) -> &[Json] {
    settings
        .get("hooks")
        .and_then(|hooks| hooks.get("SessionStart"))
        .and_then(Json::as_array)
        .unwrap_or_default()
}

/// The group this `mg` writes: no matcher, so it runs however the session
/// started.
fn group(agent: Agent) -> Json {
    Json::Object(vec![(
        "hooks".into(),
        Json::Array(vec![Json::Object(vec![
            ("type".into(), Json::String("command".into())),
            ("command".into(), Json::String(command(agent))),
            ("timeout".into(), Json::Number(TIMEOUT.into())),
        ])]),
    )])
}

fn handlers(group: &Json) -> &[Json] {
    group
        .get("hooks")
        .and_then(Json::as_array)
        .unwrap_or_default()
}

fn is_ours(handler: &Json) -> bool {
    handler
        .get("command")
        .and_then(Json::as_str)
        .is_some_and(|command| command.contains(MARKER))
}

fn has_ours(group: &Json) -> bool {
    handlers(group).iter().any(is_ours)
}

/// `group` without our handlers, or `None` when they were all it ran.
fn without_ours(mut group: Json) -> Option<Json> {
    if !has_ours(&group) {
        return Some(group);
    }
    let kept: Vec<Json> = handlers(&group)
        .iter()
        .filter(|handler| !is_ours(handler))
        .cloned()
        .collect();
    if kept.is_empty() {
        return None;
    }
    *group.entry("hooks").expect("a group with handlers") = Json::Array(kept);
    Some(group)
}

#[cfg(test)]
#[path = "hook_tests.rs"]
mod tests;
