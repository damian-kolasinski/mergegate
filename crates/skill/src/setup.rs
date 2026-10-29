//! Installing, updating and removing the skill stub and session hook in each
//! agent. Each item is reported with what happened to it; one that fails
//! leaves the rest to go ahead.

use std::fs;
use std::io::{self, Write as _};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::agent::Agent;
use crate::folder;
use crate::guide::NAME;
use crate::hook;
use crate::json::Json;
use crate::status::{Scope, State, is_ours, read_settings, skill_state};
use crate::stub;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Skill,
    Hook,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Installed,
    Updated,
    Unchanged,
    Removed,
    /// There was nothing of ours to remove.
    Absent,
    /// A skill named mergegate that no `mg` wrote, left as it was; `force`
    /// replaces it.
    Conflict,
    /// The same, which removing leaves alone.
    Kept,
    /// The agent can't hold this item.
    Unsupported,
    /// Why it failed; nothing was changed.
    Failed(String),
}

impl Change {
    /// Whether the operator has something left to sort out.
    pub fn failed(&self) -> bool {
        matches!(self, Change::Conflict | Change::Failed(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The agents that read this item; agents that share a folder share
    /// its step.
    pub agents: Vec<Agent>,
    pub kind: Kind,
    pub path: Option<PathBuf>,
    pub change: Change,
}

impl Step {
    /// What the operator does before a hook this step wrote runs.
    pub fn approval(&self) -> Option<&'static str> {
        match (self.kind, &self.change) {
            (Kind::Hook, Change::Installed | Change::Updated) => {
                self.agents.iter().find_map(|agent| agent.hook_approval())
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Install {
        force: bool,
    },
    /// Brings up to date only what an `mg` installed.
    Refresh,
}

/// Installs the stub and the hook into `agents`, replacing what an older
/// `mg` installed there. With `dry_run` it only says what it would do.
pub fn setup(scope: &Scope, agents: &[Agent], force: bool, dry_run: bool) -> Vec<Step> {
    apply(scope, agents, Mode::Install { force }, dry_run)
}

/// Updates the stubs and hooks an older `mg` installed, in any agent, and
/// installs nothing new: for when this `mg` starts its hub, so an upgrade
/// needs no setup. Only the steps that did something.
pub fn refresh(scope: &Scope) -> Vec<Step> {
    apply(scope, &Agent::ALL, Mode::Refresh, false)
        .into_iter()
        .filter(|step| !matches!(step.change, Change::Unchanged))
        .collect()
}

fn apply(scope: &Scope, agents: &[Agent], mode: Mode, dry_run: bool) -> Vec<Step> {
    let mut steps = Vec::new();
    // The folders that hold the stub once this is done.
    let mut stubbed = Vec::new();
    for (path, readers) in skill_folders(scope, agents) {
        let change = match (skill_state(&path), mode) {
            (State::Current, _) => Change::Unchanged,
            (State::Missing | State::Foreign, Mode::Refresh) => continue,
            // A copy the operator may have edited goes only when they run
            // setup themselves.
            (State::Outdated, Mode::Refresh) if !refreshable(&path) => continue,
            (State::Foreign, Mode::Install { force: false }) => Change::Conflict,
            (State::Unreadable(why), _) => Change::Failed(why),
            (State::Missing, _) => write_skill(&path, Change::Installed, dry_run),
            (State::Outdated | State::Foreign | State::Unsupported, _) => {
                write_skill(&path, Change::Updated, dry_run)
            }
        };
        if matches!(
            change,
            Change::Unchanged | Change::Installed | Change::Updated
        ) {
            stubbed.push(path.clone());
        }
        steps.push(Step {
            agents: readers,
            kind: Kind::Skill,
            path: Some(path),
            change,
        });
    }
    for &agent in agents {
        // Only once the stub is where the agent reads it now, so neither a
        // refresh nor a conflict takes away the copy it still reads.
        if !stubbed.contains(&scope.skill(agent)) {
            continue;
        }
        for former in scope.former_skills(agent).filter(|p| is_ours(p)) {
            // Without a SKILL.md the agent no longer lists it, while wakes
            // left with its script's path still run.
            let retired = dry_run || folder::write(&former, &[wake_file()]).is_ok();
            let change = removed(retired, &former);
            steps.push(Step {
                agents: vec![agent],
                kind: Kind::Skill,
                path: Some(former),
                change,
            });
        }
    }
    for &agent in agents {
        let Some(path) = scope.hooks(agent) else {
            if mode != Mode::Refresh {
                steps.push(Step {
                    agents: vec![agent],
                    kind: Kind::Hook,
                    path: None,
                    change: Change::Unsupported,
                });
            }
            continue;
        };
        let change = match read_settings(&path) {
            Err(why) => Change::Failed(why),
            Ok(settings) => {
                let missing = settings.is_none();
                let mut settings = settings.unwrap_or(Json::Object(Vec::new()));
                match (hook::state(&settings, agent), mode) {
                    (hook::HookState::Current, _) => Change::Unchanged,
                    (hook::HookState::Missing, Mode::Refresh) => continue,
                    (state, _) => {
                        let done = match state {
                            hook::HookState::Missing => Change::Installed,
                            _ => Change::Updated,
                        };
                        match hook::install(&mut settings, agent) {
                            Err(why) => Change::Failed(why.into()),
                            Ok(()) if dry_run => done,
                            Ok(()) => match write_settings(&path, &settings, missing) {
                                Ok(()) => done,
                                Err(e) => Change::Failed(e.to_string()),
                            },
                        }
                    }
                }
            }
        };
        steps.push(Step {
            agents: vec![agent],
            kind: Kind::Hook,
            path: Some(path),
            change,
        });
    }
    steps
}

/// Removes the stub and hook from `agents`, and the copies older `mg`s
/// linked for them, leaving everything else as it was.
pub fn remove(scope: &Scope, agents: &[Agent], dry_run: bool) -> Vec<Step> {
    let mut steps = Vec::new();
    for (path, _) in skill_folders(scope, agents) {
        // Every agent that reads the folder loses it, asked for or not.
        let readers = Agent::ALL
            .into_iter()
            .filter(|&agent| scope.skill(agent) == path)
            .collect();
        let change = match skill_state(&path) {
            State::Current | State::Outdated => {
                removed(dry_run || folder::remove(&path).is_ok(), &path)
            }
            State::Missing => Change::Absent,
            State::Foreign => Change::Kept,
            State::Unreadable(why) => Change::Failed(why),
            State::Unsupported => continue,
        };
        steps.push(Step {
            agents: readers,
            kind: Kind::Skill,
            path: Some(path),
            change,
        });
    }
    for &agent in agents {
        let formers = scope.former_skills(agent);
        for former in formers.filter(|p| is_ours(p) || retired(p)) {
            let change = removed(dry_run || folder::remove(&former).is_ok(), &former);
            steps.push(Step {
                agents: vec![agent],
                kind: Kind::Skill,
                path: Some(former),
                change,
            });
        }
    }
    for &agent in agents {
        let Some(path) = scope.hooks(agent) else {
            continue;
        };
        let change = match read_settings(&path) {
            Err(why) => Change::Failed(why),
            Ok(None) => Change::Absent,
            Ok(Some(mut settings)) => match hook::remove(&mut settings) {
                Err(why) => Change::Failed(why.into()),
                Ok(false) => Change::Absent,
                Ok(true) if dry_run => Change::Removed,
                Ok(true) => match write_settings(&path, &settings, false) {
                    Ok(()) => Change::Removed,
                    Err(e) => Change::Failed(e.to_string()),
                },
            },
        };
        steps.push(Step {
            agents: vec![agent],
            kind: Kind::Hook,
            path: Some(path),
            change,
        });
    }
    steps
}

/// Each skill folder `agents` read, with the agents reading it, in the
/// order of [`Agent::ALL`].
fn skill_folders(scope: &Scope, agents: &[Agent]) -> Vec<(PathBuf, Vec<Agent>)> {
    let mut folders: Vec<(PathBuf, Vec<Agent>)> = Vec::new();
    for agent in Agent::ALL.into_iter().filter(|a| agents.contains(a)) {
        let path = scope.skill(agent);
        match folders.iter_mut().find(|(p, _)| *p == path) {
            Some((_, readers)) => readers.push(agent),
            None => folders.push((path, vec![agent])),
        }
    }
    folders
}

/// Whether a refresh may replace the skill at `path`: an older stub, or a
/// link, whose target stays. Not a folder `mg skill install` wrote, which
/// the operator may have copied in and edited.
fn refreshable(path: &Path) -> bool {
    path.is_symlink()
        || fs::read_to_string(path.join("SKILL.md")).is_ok_and(|text| stub::is_stub(&text))
}

fn write_skill(path: &Path, done: Change, dry_run: bool) -> Change {
    if dry_run {
        return done;
    }
    match folder::write(path, &[("SKILL.md", &stub::text()), wake_file()]) {
        Ok(()) => done,
        Err(e) => Change::Failed(format!("writing {}: {e}", path.display())),
    }
}

fn wake_file() -> (&'static str, &'static str) {
    (stub::WAKE_SCRIPT, stub::wake_script())
}

/// Whether `path` is a former copy setup retired: a folder holding only
/// the wake script, as setup wrote it.
fn retired(path: &Path) -> bool {
    let only = |dir: &Path, name: &str| {
        fs::read_dir(dir).is_ok_and(|entries| {
            let names: Vec<_> = entries.flatten().map(|e| e.file_name()).collect();
            names == [name]
        })
    };
    let script = path.join(stub::WAKE_SCRIPT);
    !path.is_symlink()
        && only(path, "scripts")
        && only(&path.join("scripts"), "codex_wake.py")
        && fs::read_to_string(script).is_ok_and(|text| text == stub::wake_script())
}

fn removed(ok: bool, path: &Path) -> Change {
    match ok {
        true => Change::Removed,
        false => Change::Failed(format!("couldn't remove {}", path.display())),
    }
}

/// Swaps `settings` in for the file at `path`, through a link to it if it
/// is one, as a dotfiles checkout keeps them, and with its permissions;
/// `new` when there was no file, which gets the usual ones.
fn write_settings(path: &Path, settings: &Json, new: bool) -> io::Result<()> {
    let path = match new {
        true => path.to_owned(),
        false => fs::canonicalize(path)?,
    };
    let dir = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    fs::create_dir_all(dir)?;
    let mode = match fs::metadata(&path) {
        Ok(meta) => meta.permissions().mode(),
        Err(_) => 0o644,
    };
    let mut file = tempfile::Builder::new()
        .prefix(&format!(".{NAME}-"))
        .tempfile_in(dir)?;
    file.write_all(settings.print().as_bytes())?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(mode))?;
    file.persist(&path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
