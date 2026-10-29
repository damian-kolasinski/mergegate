//! `mg skill setup`, `status` and `remove`: the skill stub and session hook
//! in each coding agent, for the user's account or one repository.

use std::path::Path;

use skill::{Agent, Change, Kind, Scope, State, Step};

use super::AgentArg;
use crate::output::{CliError, Outcome, exit};
use crate::repo;

pub(super) fn setup(
    named: &[AgentArg],
    project: bool,
    dry_run: bool,
    force: bool,
) -> Result<Outcome, CliError> {
    let scope = scope(project)?;
    let agents = match named.is_empty() {
        true => scope.found(),
        false => named.iter().map(|&arg| arg.into()).collect(),
    };
    if agents.is_empty() {
        let text = "found no agent to set up: Claude Code, Codex and OpenCode keep their \
                    settings in ~/.claude, ~/.codex and ~/.config/opencode; name one with \
                    --agent to set it up all the same";
        return Ok(Outcome::done(&hub::agents::report(dry_run, &[]), text));
    }
    let steps = skill::setup(&scope, &agents, force, dry_run);
    let mut text = steps_text(&steps, dry_run);
    let wrote = steps
        .iter()
        .any(|s| matches!(s.change, Change::Installed | Change::Updated));
    if wrote && !dry_run {
        text.push_str("\n\nNew sessions load it; those running now don't.");
    }
    if steps
        .iter()
        .any(|s| s.change == Change::Conflict && s.kind == Kind::Skill)
    {
        text.push_str(
            "\nA skill named mergegate that mg didn't write is in the way: \
             mg skill setup --force replaces it.",
        );
    }
    Ok(finish(hub::agents::report(dry_run, &steps), text, &steps))
}

pub(super) fn remove(
    named: &[AgentArg],
    project: bool,
    dry_run: bool,
) -> Result<Outcome, CliError> {
    let scope = scope(project)?;
    let agents = match named.is_empty() {
        true => Agent::ALL.to_vec(),
        false => named.iter().map(|&arg| arg.into()).collect(),
    };
    let steps = skill::remove(&scope, &agents, dry_run);
    let text = steps_text(&steps, dry_run);
    Ok(finish(hub::agents::report(dry_run, &steps), text, &steps))
}

pub(super) fn status(project: bool) -> Result<Outcome, CliError> {
    let status = skill::status(&scope(project)?);
    let mut lines = Vec::new();
    for agent in &status {
        let found = if agent.found { "found" } else { "not found" };
        lines.push(format!("{} ({found})", agent.agent.label()));
        lines.push(item_line("skill", &agent.skill));
        lines.push(item_line("hook", &agent.hook));
    }
    if status.iter().any(|a| a.found && !a.ready()) {
        lines.push(String::new());
        lines.push("mg skill setup sets up the agents found.".to_owned());
    }
    Ok(Outcome::done(
        &hub::agents::to_api(&status),
        lines.join("\n"),
    ))
}

/// The user's agents, or the current repository's for `--project`.
fn scope(project: bool) -> Result<Scope, CliError> {
    let user = hub::agents::user_scope()
        .ok_or_else(|| CliError::new("no_home", "HOME isn't set, so mg can't find your agents"))?;
    if !project {
        return Ok(user);
    }
    let root = repo::read(&["rev-parse", "--show-toplevel"])?.ok_or_else(|| {
        CliError::new(
            "not_a_repo",
            "the current directory is not inside a git repository",
        )
        .hint("run mg skill setup --project in the repository to set up")
    })?;
    Ok(Scope::project(root.into(), home()))
}

fn home() -> std::path::PathBuf {
    std::env::var_os("HOME").unwrap_or_default().into()
}

fn finish(report: api::agent_setup::SetupReport, text: String, steps: &[Step]) -> Outcome {
    let outcome = Outcome::done(&report, text);
    match steps.iter().any(|s| s.change.failed()) {
        true => outcome.exit(exit::ERROR),
        false => outcome,
    }
}

fn steps_text(steps: &[Step], dry_run: bool) -> String {
    let mut lines = Vec::new();
    if dry_run {
        lines.push("Dry run: nothing was changed. This is what would happen.".to_owned());
    }
    for step in steps {
        let agents: Vec<_> = step.agents.iter().map(|a| a.label()).collect();
        let kind = match step.kind {
            Kind::Skill => "skill",
            Kind::Hook => "hook",
        };
        let (change, note) = match &step.change {
            Change::Installed => ("installed", None),
            Change::Updated => ("updated", None),
            Change::Unchanged => ("unchanged", None),
            Change::Removed => ("removed", None),
            Change::Absent => ("absent", None),
            Change::Conflict => ("conflict", Some("not written by mg; left as it was".into())),
            Change::Kept => ("kept", Some("not written by mg; left as it was".into())),
            Change::Unsupported => ("unsupported", None),
            Change::Failed(why) => ("failed", Some(why.clone())),
        };
        let at = match &step.path {
            Some(path) => tilde(path),
            None => format!("{} runs no session hooks", agents.join(", ")),
        };
        lines.push(format!(
            "{kind:5}  {change:11}  {at}  ({})",
            agents.join(", ")
        ));
        for note in note.into_iter().chain(step.approval().map(str::to_owned)) {
            lines.push(format!("{:5}  {:11}  {note}", "", ""));
        }
    }
    lines.join("\n")
}

fn item_line(kind: &str, item: &skill::Item) -> String {
    let state = match &item.state {
        State::Current => "current",
        State::Outdated => "outdated",
        State::Missing => "missing",
        State::Foreign => "foreign",
        State::Unreadable(_) => "unreadable",
        State::Unsupported => "unsupported",
    };
    let mut line = format!("  {kind:5}  {state:11}");
    if let Some(path) = &item.path {
        line.push_str(&format!("  {}", tilde(path)));
    }
    if let State::Unreadable(why) = &item.state {
        line.push_str(&format!(": {why}"));
    }
    line.trim_end().to_owned()
}

/// `path` with the user's home as `~`.
fn tilde(path: &Path) -> String {
    match path.strip_prefix(home()) {
        Ok(rest) if !home().as_os_str().is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}
