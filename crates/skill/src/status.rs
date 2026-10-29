//! Where each agent's skill and session hook stand, which `mg skill status`,
//! `mg doctor` and the hub's UI report and setup acts on.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::agent::Agent;
use crate::guide::NAME;
use crate::hook::{self, HookState};
use crate::json::Json;
use crate::stub;

/// Whose agents setup works on: the user's, or one repository's.
#[derive(Debug, Clone)]
pub struct Scope {
    /// What the agents' paths are relative to.
    base: PathBuf,
    /// The user's home, where agents keep the settings that show they are
    /// installed.
    home: PathBuf,
}

impl Scope {
    pub fn user(home: PathBuf) -> Self {
        Self {
            base: home.clone(),
            home,
        }
    }

    pub fn project(root: PathBuf, home: PathBuf) -> Self {
        Self { base: root, home }
    }

    /// The agents installed on this machine, in [`Agent::ALL`]'s order.
    pub fn found(&self) -> Vec<Agent> {
        Agent::ALL
            .into_iter()
            .filter(|agent| agent.found(&self.home))
            .collect()
    }

    pub(crate) fn skill(&self, agent: Agent) -> PathBuf {
        self.base.join(agent.skills()).join(NAME)
    }

    /// The places an older `mg` linked the skill for `agent`, except one
    /// that is where it reads it now, as when the operator linked
    /// `~/.codex/skills` to `~/.agents/skills`.
    pub(crate) fn former_skills(&self, agent: Agent) -> impl Iterator<Item = PathBuf> + '_ {
        let current = fs::canonicalize(self.base.join(agent.skills())).ok();
        agent.former_skills().iter().filter_map(move |dir| {
            let folder = self.base.join(dir);
            let same = current.is_some() && fs::canonicalize(&folder).ok() == current;
            (!same).then(|| folder.join(NAME))
        })
    }

    pub(crate) fn hooks(&self, agent: Agent) -> Option<PathBuf> {
        agent.hooks().map(|file| self.base.join(file))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Installed as this `mg` installs it.
    Current,
    /// Installed by another `mg`, or changed since.
    Outdated,
    Missing,
    /// A skill named mergegate that no `mg` wrote.
    Foreign,
    /// Why it couldn't be read.
    Unreadable(String),
    /// The agent can't hold it, as an agent without command hooks a hook.
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// `None` for an item the agent can't hold.
    pub path: Option<PathBuf>,
    pub state: State,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    pub agent: Agent,
    pub found: bool,
    pub skill: Item,
    pub hook: Item,
}

impl AgentStatus {
    /// Whether setup has nothing left to do for it.
    pub fn ready(&self) -> bool {
        self.skill.state == State::Current
            && matches!(self.hook.state, State::Current | State::Unsupported)
    }
}

/// Every agent `mg` knows, found or not.
pub fn status(scope: &Scope) -> Vec<AgentStatus> {
    Agent::ALL
        .into_iter()
        .map(|agent| AgentStatus {
            agent,
            found: agent.found(&scope.home),
            skill: skill(scope, agent),
            hook: hook(scope, agent),
        })
        .collect()
}

fn skill(scope: &Scope, agent: Agent) -> Item {
    let path = scope.skill(agent);
    let mut state = skill_state(&path);
    // The agent lists a copy an older mg linked elsewhere beside this one.
    if state == State::Current && scope.former_skills(agent).any(|p| is_ours(&p)) {
        state = State::Outdated;
    }
    Item {
        path: Some(path),
        state,
    }
}

fn hook(scope: &Scope, agent: Agent) -> Item {
    let Some(path) = scope.hooks(agent) else {
        return Item {
            path: None,
            state: State::Unsupported,
        };
    };
    let state = match read_settings(&path) {
        Ok(None) => State::Missing,
        Ok(Some(settings)) => match hook::state(&settings, agent) {
            HookState::Current => State::Current,
            HookState::Outdated => State::Outdated,
            HookState::Missing => State::Missing,
        },
        Err(why) => State::Unreadable(why),
    };
    Item {
        path: Some(path),
        state,
    }
}

/// Where the skill folder at `path` stands, links followed.
pub(crate) fn skill_state(path: &Path) -> State {
    let link = match fs::symlink_metadata(path) {
        Ok(meta) => meta.file_type().is_symlink(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return State::Missing,
        Err(e) => return State::Unreadable(e.to_string()),
    };
    match fs::read_to_string(path.join("SKILL.md")) {
        Ok(text) if text == stub::text() => State::Current,
        Ok(text) if stub::is_ours(&text) => State::Outdated,
        Ok(_) => State::Foreign,
        // A link to a folder that's gone, as an older mg's skill whose data
        // directory was removed, holds nobody's skill.
        Err(e) if e.kind() == io::ErrorKind::NotFound && link && !path.exists() => State::Outdated,
        Err(e) if e.kind() == io::ErrorKind::NotFound => State::Foreign,
        Err(e) => State::Unreadable(e.to_string()),
    }
}

pub(crate) fn is_ours(path: &Path) -> bool {
    matches!(skill_state(path), State::Current | State::Outdated)
}

/// An agent's settings file, `None` when there is none yet; `Err` says why
/// it can't be edited.
pub(crate) fn read_settings(path: &Path) -> Result<Option<Json>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    match Json::parse(&text) {
        Ok(settings @ Json::Object(_)) => Ok(Some(settings)),
        Ok(_) => Err("isn't a JSON object".into()),
        Err(e) => Err(format!("isn't valid JSON: {e}")),
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
